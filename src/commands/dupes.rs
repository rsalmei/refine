use crate::commands::Refine;
use crate::entries::{Entry, InputInfo, TraversalMode};
use crate::utils::{self, display_abort};
use anyhow::Result;
use clap::{Args, ValueEnum};
use deunicode::deunicode;
use human_repr::HumanCount;
use mime_guess::MimeGuess;
use rayon::prelude::*;
use regex::Regex;
use std::boxed::Box;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

// TODO find some way to mark files/groups as "not a dupe".

#[derive(Debug, Args)]
pub struct Dupes {
    /// Identical (size and sample), or similar (rare tokens and fuzzy matching).
    #[arg(short = 'm', long, default_value_t = SearchMode::All, value_name = "STR", value_enum)]
    mode: SearchMode,
    /// Sample size in kbytes (0 to disable).
    #[arg(short = 's', long, default_value_t = 4, value_name = "INT")]
    sample: usize,
    /// The threshold for similarity checks (0.0 to 1.0).
    #[arg(short = 't', long, default_value_t = 0.7, value_name = "FLOAT")]
    threshold: f64,
    /// Show the cleaned filenames for similarity checks.
    #[arg(short = 'v', long)]
    verbose: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum SearchMode {
    #[value(alias = "i")]
    Identical,
    #[value(alias = "s")]
    Similar,
    #[value(alias = "a")]
    All,
}

#[derive(Debug)]
pub struct Media {
    entry: Entry,
    size: u64,
    cleaned_name: String,              // cleaned name for similarity checks.
    kind: &'static str,                // guessed from both the MIME type and the file extension.
    sample: Option<Option<Box<[u8]>>>, // only populated if needed, and double to remember when already tried.
}

impl Refine for Dupes {
    type Media = Media;
    const OPENING_LINE: &'static str = "Detect duplicate files";
    const T_MODE: TraversalMode = TraversalMode::Files;

    fn tweak(&mut self, _: &InputInfo) {
        if self.threshold < 0.0 || self.threshold > 1.0 {
            self.threshold = self.threshold.clamp(0.0, 1.0);
            eprintln!(
                "warning: invalid similarity threshold, using {:.1}",
                self.threshold
            );
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let (mut by_size, mut by_name) = (0, 0);

        // step: detect duplicates by content.
        if let SearchMode::Identical | SearchMode::All = self.mode {
            println!("by identical size and {}KB sample:", self.sample);
            by_size = self.find_identical(&mut medias, |size, g| {
                println!("\n{} x{}", size.human_count_bytes(), g.len());
                g.iter().for_each(|&m| println!("{}", m.entry));
            });
            if by_size == 0 {
                println!("\nnone found!");
            }
            println!();
        }

        // step: detect duplicates by name.
        if let SearchMode::Similar | SearchMode::All = self.mode {
            println!("by name similarity:");
            by_name = self.find_similar(&medias, |sim, g| {
                println!("\n{sim:.1}% similar x{}", g.len());
                let show = if self.verbose {
                    |m: &Media, s| println!("{s:>7}: {} [{}]", m.entry, m.cleaned_name)
                } else {
                    |m: &Media, s| println!("{s:>7}: {}", m.entry)
                };
                for m in g {
                    let s = m.size.human_count_bytes().to_string(); // TODO: wait for human_repr to support size.
                    show(m, s);
                }
            });
            if by_name == 0 {
                println!("\nnone found!");
            }
            println!();
        }

        // step: display a summary receipt.
        let total = medias.len();
        println!("total files: {total}");
        if let SearchMode::Identical | SearchMode::All = self.mode {
            println!("  by size: {by_size} dupes{}", display_abort(by_name == 0));
        }
        if let SearchMode::Similar | SearchMode::All = self.mode {
            println!("  by name: {by_name} dupes{}", display_abort(true));
        }
        Ok(())
    }
}

impl Dupes {
    /// Find identical files based on size and sample checks.
    fn find_identical<FS>(&self, medias: &mut [Media], show: FS) -> usize
    where
        FS: Fn(u64, Vec<&Media>),
    {
        let group = |m: &Media| (Reverse(m.size), m.kind);
        medias.sort_by_cached_key(group);
        medias
            .chunk_by_mut(|m, m2| group(m) == group(m2))
            .filter(|_| utils::is_running())
            .filter(|g| g.len() > 1)
            .flat_map(|g| {
                g.iter_mut().for_each(|m| {
                    m.cache_sample(self.sample * 1024); // warm up samples for groups with at least 2 files.
                });
                let mut split = HashMap::with_capacity(g.len());
                g.iter()
                    .map(|m| (m, m.sample.as_ref().unwrap())) // sample is always populated by cache_sample.
                    .for_each(|(m, sample)| split.entry(sample).or_insert_with(Vec::new).push(m));
                split.into_values().filter(|v| v.len() > 1)
            })
            .map(|mut g| {
                g.sort_unstable_by(|m, n| m.entry.cmp(&n.entry));
                show(g[0].size, g);
            })
            .count()
    }

    /// Find similar files based on name similarity.
    fn find_similar<FS>(&self, medias: &[Media], show: FS) -> usize
    where
        FS: Fn(f64, Vec<&Media>),
    {
        // build token frequency map for rare token scoring.
        let token_freq = medias
            .iter()
            .flat_map(|m| m.cleaned_name.split_ascii_whitespace())
            .fold(HashMap::new(), |mut acc, token| {
                *acc.entry(token).or_insert(0) += 1;
                acc
            });

        // pre-calculate token sets for each media.
        let media_token_sets = medias
            .iter()
            .map(|m| {
                m.cleaned_name
                    .split_ascii_whitespace()
                    .collect::<HashSet<_>>()
            })
            .collect::<Vec<_>>();

        // build inverted index for tokens.
        let mut token_blocks = HashMap::new();
        medias.iter().enumerate().for_each(|(i, media)| {
            media
                .cleaned_name
                .split_ascii_whitespace()
                .for_each(|token| token_blocks.entry(token).or_insert_with(Vec::new).push(i));
        });

        // setup union-find.
        let mut parent = (0..medias.len()).collect::<Vec<_>>();
        let mut group_sim = HashMap::new(); // root -> (sum, count)
        fn find(parent: &mut [usize], x: usize) -> usize {
            if parent[x] != x {
                parent[x] = find(parent, parent[x]);
            }
            parent[x]
        }
        fn union(
            parent: &mut [usize],
            group_sim: &mut HashMap<usize, (f64, usize)>,
            x: usize,
            y: usize,
            sim: f64,
        ) {
            let xr = find(parent, x);
            let yr = find(parent, y);
            if xr != yr {
                // merge groups and update sum/count.
                let (sum1, count1) = group_sim.remove(&xr).unwrap_or((0.0, 0));
                let (sum2, count2) = group_sim.remove(&yr).unwrap_or((0.0, 0));
                parent[yr] = xr;
                group_sim.insert(xr, (sum1 + sum2 + sim, count1 + count2 + 1));
            } else {
                // update sum/count for the group.
                let entry = group_sim.entry(xr).or_insert((0.0, 0));
                entry.0 += sim;
                entry.1 += 1;
            }
        }

        // prepare to compare pairs of media.
        let total_pairs = {
            let mut seen_pairs = HashSet::new();
            token_blocks
                .values()
                .flat_map(|g| {
                    (0..g.len()).flat_map(move |i| (i + 1..g.len()).map(move |j| (g[i], g[j])))
                })
                .filter(|_| utils::is_running())
                .filter(|&(a, b)| seen_pairs.insert((a.min(b), a.max(b))))
                .count()
        };

        // compare each unique pair only once and in parallel.
        const SPINNER: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
        let spinner_len = SPINNER.chars().count();
        let counter = Arc::new(AtomicUsize::new(0));
        let spin_counter = Arc::new(AtomicUsize::new(0)); // a separate counter for the spinner animation.
        let seen_pairs = Arc::new(Mutex::new(HashSet::new()));
        let progress_state = Arc::new(Mutex::new((Instant::now(), -1))); // contains (last_update, last_percent).
        let similar = token_blocks
            .values()
            .par_bridge()
            .flat_map_iter(|g| {
                (0..g.len()).flat_map(move |i| (i + 1..g.len()).map(move |j| (g[i], g[j])))
            })
            .filter(|_| utils::is_running())
            .filter(|&(a, b)| seen_pairs.lock().unwrap().insert((a.min(b), a.max(b)))) // the mutex is not expected to be poisoned.
            .inspect(|_| {
                let count = counter.fetch_add(1, AtomicOrdering::Relaxed);
                let mut state = progress_state.lock().unwrap(); // the mutex is not expected to be poisoned.
                let (last_update, last_percent) = *state;
                let percent = (count as f64 / total_pairs as f64 * 100.0) as i32;

                // update if time has passed or a % threshold is crossed, and if progress has advanced.
                if (last_update.elapsed() > Duration::from_millis(100)
                    || percent / 5 > last_percent / 5)
                    && percent >= last_percent
                {
                    let spin_idx = spin_counter.fetch_add(1, AtomicOrdering::Relaxed);
                    let spin = SPINNER.chars().nth(spin_idx % spinner_len).unwrap(); // spinner_len is non-zero.
                    eprint!("\r{spin} {percent:.0}%");
                    *state = (Instant::now(), percent);
                }
            })
            .filter(|&(a, b)| medias[a].kind == medias[b].kind)
            .filter(|&(a, b)| {
                // ensure there's at least one shared non-numeric token.
                media_token_sets[a]
                    .intersection(&media_token_sets[b])
                    .any(|token| token.chars().any(|c| !c.is_ascii_digit()))
            })
            .filter_map(|(a, b)| {
                let clean1 = &medias[a].cleaned_name;
                let clean2 = &medias[b].cleaned_name;
                let sim = {
                    let lev = strsim::normalized_levenshtein(clean1, clean2);
                    let dice = strsim::sorensen_dice(clean1, clean2);
                    let rare_token_boost = rare_token_similarity(clean1, clean2, &token_freq);
                    // combine all three metrics: 40% string similarity, 60% rare token similarity.
                    (lev.max(dice) * 0.4) + (rare_token_boost * 0.6)
                };
                (sim >= self.threshold).then_some((a, b, sim))
            })
            .collect::<Vec<_>>();
        eprint!("\r      \r"); // clear spinner/percent.

        // sequentially union similar pairs.
        similar.into_iter().for_each(|(a, b, sim)| {
            union(&mut parent, &mut group_sim, a, b, sim);
        });

        // collect groups by root.
        let mut groups = HashMap::new();
        (0..medias.len()).for_each(|i| {
            let root = find(&mut parent, i);
            groups.entry(root).or_insert(vec![]).push(i);
        });

        // collect groups with more than one member, and filter out sequential ones.
        let mut group_infos = groups
            .values()
            .filter(|g| g.len() > 1)
            .map(|g| {
                // collect group medias.
                let group_medias = g.iter().map(|&idx| &medias[idx]).collect::<Vec<_>>();
                let root = find(&mut parent, g[0]);
                // safe unwrap: group_sim always has an entry for each root.
                let (sum, count) = group_sim.get(&root).copied().unwrap_or((0.0, 1));
                let avg_sim = if count > 0 { sum / count as f64 } else { 1.0 };
                (avg_sim, group_medias)
            })
            .filter(|(_, g)| {
                // check for TV series, episode sequences, etc., and hide them.
                !is_likely_sequential(g)
            })
            .collect::<Vec<_>>();

        // sort groups by average similarity in descending order.
        group_infos.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

        // display each group.
        group_infos
            .into_iter()
            .map(|(avg_sim, mut g)| {
                g.sort_unstable_by(|m, n| m.entry.cmp(&n.entry));
                show(avg_sim * 100.0, g);
            })
            .count()
    }
}

/// Check if a group of files looks like episodes from a TV series or a sequence.
/// If it is, it is not considered a group of duplicates.
fn is_likely_sequential(group: &[&Media]) -> bool {
    // simple pattern to extract all numbers from filenames.
    static NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

    if group.len() < 2 {
        return false; // not a series if less than 2 files.
    }

    // extract number sequences from each filename.
    let number_sequences = group
        .iter()
        .map(|m| {
            NUMBERS
                .find_iter(&m.cleaned_name)
                .map(|m| m.as_str().parse::<i64>().unwrap_or(-1)) // parse numbers, fallback to -1.
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    // filter out files that do not contain any numbers.
    let sequences_with_numbers = number_sequences
        .iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    // allow a small number of files without numbers (e.g., a base file and its numbered extras).
    let files_without_numbers = group.len() - sequences_with_numbers.len();
    if files_without_numbers > 1 && files_without_numbers as f64 / group.len() as f64 > 0.1 {
        return false;
    }

    // find the most common length of number sequences.
    let mut lengths = HashMap::new();
    for seq in &sequences_with_numbers {
        *lengths.entry(seq.len()).or_insert(0) += 1;
    }
    let common_len = lengths.into_iter().max_by_key(|&(_, count)| count);

    // if no common length, or common length is zero, it's not a clear sequence.
    let (len, _) = match common_len {
        // use the most common length, even if it appears only once.
        Some((len, count)) if len > 0 && count >= 1 => (len, count),
        _ => return false,
    };

    // filter for sequences with a length close to the common length.
    let sequences_with_common_len = sequences_with_numbers
        .iter()
        .filter(|s| s.len().abs_diff(len) <= 1)
        .collect::<Vec<_>>();

    // not a series if not enough files match the common length.
    if sequences_with_common_len.len() < 2 {
        return false;
    }

    // count how many number positions are constant vs. varying.
    let mut varying_indices = HashSet::new();
    for i in 0..len {
        let mut values = HashSet::new();
        for seq in &sequences_with_common_len {
            // check if the index is valid for the current sequence.
            if let Some(&val) = seq.get(i) {
                values.insert(val);
            }
        }
        if values.len() > 1 {
            varying_indices.insert(i);
        }
    }

    // it's a series if at least one number varies.
    !varying_indices.is_empty()
}

/// Calculates similarity between two strings based on rare tokens.
fn rare_token_similarity(a: &str, b: &str, token_freq: &HashMap<&str, usize>) -> f64 {
    let a_tokens = a.split_ascii_whitespace().collect::<HashSet<_>>();
    let b_tokens = b.split_ascii_whitespace().collect::<HashSet<_>>();

    // calculate the weighted score for a set of tokens.
    let score = |tokens: &HashSet<&str>| -> f64 {
        tokens
            .iter()
            .map(|token| {
                let freq = token_freq.get(token).copied().unwrap_or(1);
                1.0 / (freq as f64).ln_1p() // the score is the inverse of the log of frequency.
            })
            .sum()
    };

    let a_score = score(&a_tokens);
    let b_score = score(&b_tokens);

    if a_score == 0.0 || b_score == 0.0 {
        return 0.0;
    }

    let intersection = a_tokens.intersection(&b_tokens).copied().collect();
    let intersection_score = score(&intersection);

    // calculate base similarity.
    let base_sim = if a_tokens.is_subset(&b_tokens) || b_tokens.is_subset(&a_tokens) {
        // for subsets, similarity is the ratio of the intersection to the smaller set's score.
        intersection_score / a_score.min(b_score)
    } else {
        // for others, use a weighted jaccard index.
        let union_score = a_score + b_score - intersection_score;
        if union_score == 0.0 {
            return if intersection_score > 0.0 { 1.0 } else { 0.0 };
        }
        intersection_score / union_score
    };

    // penalize based on the difference in token count.
    let len_a = a_tokens.len() as f64;
    let len_b = b_tokens.len() as f64;
    let length_ratio = len_a.min(len_b) / len_a.max(len_b);

    // use a stricter penalty for few shared tokens, and a more lenient one for more shared tokens.
    let shared_tokens = a_tokens.intersection(&b_tokens).count();
    let exponent = if shared_tokens <= 1 { 0.6 } else { 1.0 / 3.0 };
    let penalty = length_ratio.powf(exponent);

    base_sim * penalty
}

impl Media {
    fn cache_sample(&mut self, size: usize) {
        if self.sample.is_none() {
            let grab_sample = || {
                let mut file = File::open(&self.entry)?;
                let file_len = self.size;

                if file_len <= size as u64 {
                    // read the whole file if it's smaller than the sample size.
                    let mut buf = Vec::with_capacity(file_len as usize);
                    file.read_to_end(&mut buf)?;
                    return Ok::<_, io::Error>(buf);
                }

                // allocate buffer for all chunks.
                let mut buf = vec![0; size];
                let chunk_size = size / 3; // may not be divisible by 3, but that's okay.

                // read from the start.
                file.read_exact(&mut buf[..chunk_size])?;

                // read from the middle.
                let mid_pos = file_len / 2 - chunk_size as u64 / 2;
                file.seek(SeekFrom::Start(mid_pos))?;
                file.read_exact(&mut buf[chunk_size..chunk_size * 2])?;

                // read from the end; this last chunk must compensate for the remainder of division.
                let end_pos = file_len - (size - chunk_size * 2) as u64;
                file.seek(SeekFrom::Start(end_pos))?;
                file.read_exact(&mut buf[chunk_size * 2..])?;

                Ok(buf)
            };

            self.sample = match grab_sample() {
                Ok(buf) => Some(Some(buf.into_boxed_slice())),
                Err(err) => {
                    eprintln!("error: load sample: {err:?}.");
                    Some(None)
                }
            };
        }
    }
}

/// Cleans the filename by normalizing it, removing diacritics, and filtering out common words.
fn clean_words(name: &str) -> String {
    static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}0-9]+").unwrap()); // accented letters, digits, no underscores.
    static TAGS_MULTI: LazyLock<Regex> = LazyLock::new(|| {
        const SEP: &str = r"[ .-]?";
        const TAGS: &[&[&str]] = &[
            &["web", "dl"],
            &["blu", "ray"],
            &["(web|dvd|bd|br|hd)", "rip"],
            &["hd", "tv"],
            &["5\\.1"],
            &["6", "ch"],
            &["ac", "3"],
            &["[hx]", "26[45]"],
        ];
        Regex::new(
            &TAGS
                .iter()
                .map(|t| t.join(SEP))
                .collect::<Vec<_>>()
                .join("|"),
        )
        .unwrap()
    });
    static STOPWORDS: LazyLock<HashSet<&str>> = LazyLock::new(|| {
        #[rustfmt::skip]
        const SET: &[&str] = &[
            // non-content words, common release types, resolutions, codecs.
            "the", "a", "an", "of", "and", "in", "on", "at", "to", "by", "as",
            "e", "o", "os", "um", "uma", "uns", "umas", "ao", "aos", "à", "às", "da", "de", "do", "em", "das", "dos",
            "cam", "ts", "tc", "r5", "dvdscr", "dvdscreener",
            "repack", "limited", "internal", "remux", "fullhd", "hd", "1400mb",
            "ac", "dts", "aac", "ddp", "mp3", "1080p", "720p", "2160p", "4k", "mp4",
            "hevc", "psa", "xvid", "xvidhd", "10bit", "8bit",
        ];
        SET.iter().copied().collect()
    });

    // transliterate to ascii, removing accents and special characters.
    let base = deunicode(name).to_ascii_lowercase();

    let cleaned = TAGS_MULTI.replace_all(&base, "");
    let cleaned = WORDS
        .find_iter(&cleaned)
        .map(|m| m.as_str())
        .filter(|word| !STOPWORDS.contains(word))
        .map(|word| word.to_owned())
        .collect::<Vec<_>>();

    match cleaned.is_empty() {
        true => base,
        false => cleaned.join(" "),
    }
}

fn classify_media_kind(ext: &str) -> &'static str {
    // guess the mime type from the extension.
    let mime = MimeGuess::from_ext(ext).first_raw().unwrap_or_default();
    let top = mime.split('/').next().unwrap_or_default();

    match top {
        "video" | "audio" | "image" | "text" => top,
        "application" => match ext.to_ascii_lowercase().as_str() {
            // video extensions that are misclassified as application.
            "mkv" | "webm" | "rmvb" | "m2ts" | "mts" | "f4v" | "vob" | "ogv" => "video",
            // document.
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
            | "rtf" => "document",
            // archive.
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "lz" | "lzma" | "iso" | "cab"
            | "arj" | "z" => "archive",
            // subtitle.
            "srt" | "ass" | "ssa" | "sub" | "vtt" | "idx" | "sup" => "subtitle",
            // text (some application/* are actually text).
            "csv" | "json" | "xml" | "yaml" | "yml" | "ini" | "conf" => "text",
            _ => "application",
        },
        _ => "unknown",
    }
}

impl TryFrom<Entry> for Media {
    type Error = (Entry, anyhow::Error);

    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let (stem, ext) = entry.filename_parts();
        Ok(Media {
            size: entry.metadata().map_or(0, |m| m.len()),
            cleaned_name: clean_words(stem),
            kind: classify_media_kind(ext),
            entry,
            sample: None,
        })
    }
}
