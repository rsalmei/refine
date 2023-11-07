mod media;

use human_repr::HumanCount;
use media::Media;
use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, fs, iter};

fn main() {
    let files = env::args().skip(1);
    if files.len() == 0 {
        println!("Missing directories to scan");
        return;
    }
    let mut files = files
        .map(|s| s.parse::<PathBuf>().unwrap())
        .flat_map(entries)
        .filter_map(|pb| pb.metadata().map(|m| (pb, m.len())).ok())
        .map(Media::new)
        .collect::<Vec<_>>();

    // first by size.
    println!("\n-- by size");
    let count_size = detect_duplicates(
        &mut files,
        |m| &m.size,
        |&size, acc, count| {
            let mut split = HashMap::with_capacity(acc.len());
            acc.iter().for_each(|&m| {
                let key = m.sample().unwrap(); // FIXME remove unwrap.
                split.entry(key).or_insert_with(Vec::new).push(m)
            });
            split.values_mut().filter(|v| v.len() > 1).for_each(|v| {
                *count += 1;
                println!("\n{} x{}", size.human_count_bytes(), v.len());
                v.sort_unstable();
                v.iter().for_each(|&m| println!("{}", m.path.display()));
            })
        },
    );

    // then by name.
    println!("\n-- by name");
    let count_name = detect_duplicates(
        &mut files,
        |m| &m.words,
        |_words, acc, count| {
            *count += 1;
            println!("\nx{}", acc.len());
            acc.sort_unstable();
            acc.iter()
                .for_each(|m| println!("{}: {}", m.size.human_count_bytes(), m.path.display()));
        },
    );

    println!("\ntotal files: {}", files.len());
    println!("  by size: {count_size} duplicates");
    println!("  by name: {count_name} duplicates");
}

fn entries(pb: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
    if pb.file_name().unwrap().to_string_lossy().starts_with('.') {
        return Box::new(iter::empty());
    }
    match pb.is_dir() {
        true => match fs::read_dir(pb.as_path()) {
            Ok(reader) => Box::new(reader.flat_map(|r| match r {
                Ok(de) => entries(de.path()),
                Err(err) => {
                    eprintln!("{err:?}");
                    Box::new(iter::empty())
                }
            })),
            Err(err) => {
                eprintln!("{} -> {err:?}", pb.display());
                Box::new(iter::empty())
            }
        },
        false => Box::new(iter::once(pb)),
    }
}

/// Sort the files by groups, and apply some algorithm on each of them.
fn detect_duplicates<G, FG, ALG>(files: &mut [Media], group: FG, algo: ALG) -> usize
where
    G: PartialEq + Ord + Default + Clone,
    FG: Fn(&Media) -> &G,
    ALG: Fn(&G, &mut [&Media], &mut usize),
{
    files.sort_by_cached_key(|m| group(m).clone());
    let mut last = &Default::default();
    let (mut count, mut acc) = (0, vec![]);
    files.iter().map(Some).chain(None).for_each(|om| {
        match om {
            Some(m) if group(m) == last => return acc.push(m),
            _ if acc.len() < 2 => {}
            _ => algo(last, &mut acc, &mut count),
        }
        if let Some(m) = om {
            acc.clear();
            acc.push(m);
            last = group(m);
        }
    });
    count
}
