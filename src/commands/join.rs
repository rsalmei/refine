use crate::commands::Refine;
use crate::entries::{Entry, Fetcher, ROOT, Recurse, TraversalMode};
use crate::impl_original_entry;
use crate::media::{FileOps, NewEntry, OriginalEntry};
use crate::utils;
use anyhow::{Context, Result, anyhow};
use clap::{Args, ValueEnum};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Args)]
pub struct Join {
    /// The target directory; will be created if it doesn't exist.
    #[arg(short = 't', long, default_value = ".", value_name = "PATH")]
    target: PathBuf,
    /// The type of join to perform.
    #[arg(short = 'b', long, default_value_t = By::Move, value_name = "STR", value_enum)]
    by: By,
    /// How to resolve clashes.
    #[arg(short = 'c', long, default_value_t = Clashes::NameSequence, value_name = "STR", value_enum)]
    clashes: Clashes,
    /// Force joining already in place files and directories, i.e. in subdirectories of the target.
    #[arg(short = 'f', long)]
    force: bool,
    /// Do not remove empty parent directories after joining files.
    #[arg(short = 'p', long)]
    parents: bool,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum By {
    #[value(aliases = ["m", "mv"])]
    Move,
    #[value(aliases = ["c", "cp"])]
    Copy,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Clashes {
    #[value(aliases = ["s", "sq", "seq", "ns"])]
    NameSequence,
    #[value(aliases = ["pn"])]
    ParentName,
    #[value(aliases = ["np"])]
    NameParent,
    #[value(aliases = ["i", "ig"])]
    Ignore,
}

#[derive(Debug)]
pub struct Media {
    entry: Entry,
    new_name: Option<String>,
    skip: Skip,
}

#[derive(Debug, Clone, Copy)]
enum Skip {
    Yes,
    No,
    Target,
}

#[derive(Debug)]
struct Shared {
    target: Entry,
    force: bool,
}

static SHARED: OnceLock<Shared> = OnceLock::new();

impl Refine for Join {
    type Media = Media;
    const OPENING_LINE: &'static str = "Join files";
    const MODE: TraversalMode = TraversalMode::DirsStop;

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        if self.target.is_file() {
            return Err(anyhow!("target must be a directory or not exist"))
                .with_context(|| format!("invalid target: {:?}", self.target));
        }
        let target = Entry::try_new(&self.target, true).map_err(|(e, _)| e)?; // either a directory or doesn't exist.

        let shared = Shared {
            target: target.clone(),
            force: self.force,
        };
        SHARED.set(shared).unwrap();
        let total = medias.len();

        // step: read the target directory, which might not be empty, to detect outer clashes (not in medias).
        let mut target_names = Vec::new();
        if target.exists() {
            // if target happens to be inside any input path and is not empty, this will dup the files.
            let fetcher = Fetcher::single(&target, Recurse::Shallow);
            let in_target = fetcher.fetch(Join::HANDLES).collect::<Vec<_>>();
            target_names.extend(in_target.iter().map(|e| e.file_name().to_string()));
            medias.extend(in_target.into_iter().map(|entry| Media {
                entry,
                new_name: None,
                skip: Skip::Target,
            }));
        }

        // step: detect clashes (files with the same name in different directories), and resolve them.
        medias.sort_unstable_by(|m, n| {
            // put files already in place first.
            (m.entry.file_name(), !m.is_in_place()).cmp(&(n.entry.file_name(), !n.is_in_place()))
        });
        medias.dedup_by(|m, n| m.entry.to_str() == n.entry.to_str()); // remove target dup files.
        let mut clashes = 0;
        medias
            .chunk_by_mut(|m, n| m.entry.file_name() == n.entry.file_name())
            .filter(|g| g.len() > 1)
            .for_each(|g| {
                clashes += g.len() - 1; // one is (or will be) in target, the others are clashes.
                let (name, ext) = g[0].entry.filename_parts();
                let (name, ext) = (name.to_owned(), ext.to_owned()); // g must not be borrowed.
                let dot = if ext.is_empty() { "" } else { "." };
                match self.clashes {
                    Clashes::NameSequence => {
                        let mut seq = 2..;
                        g.iter_mut().skip(1).for_each(|m| {
                            let new_name = (&mut seq)
                                .map(|i| format!("{name}-{i}{dot}{ext}"))
                                .find(|s| target_names.iter().all(|t| s != t))
                                .unwrap();
                            m.new_name = Some(new_name);
                        })
                    }
                    Clashes::ParentName | Clashes::NameParent => g.iter_mut().for_each(|m| {
                        let par = m.entry.parent().unwrap_or(ROOT.clone());
                        let par = par.file_name();
                        if let Clashes::ParentName = self.clashes {
                            m.new_name = Some(format!("{par}-{name}{dot}{ext}"));
                        } else {
                            m.new_name = Some(format!("{name}-{par}{dot}{ext}"));
                        }
                    }),
                    Clashes::Ignore => g.iter_mut().for_each(|m| m.skip = Skip::Yes),
                }
            });

        // step: settle results by removing the files that are in place or skipped.
        medias.sort_unstable_by(|m, n| m.entry.cmp(&n.entry));
        let mut in_place = 0;
        medias.retain(|m| match (m.skip, m.is_in_place()) {
            (Skip::No, false) => true,
            (Skip::No, true) => {
                in_place += 1;
                println!("already in place: {}", m.entry);
                false
            }
            (Skip::Yes, _) => {
                println!("clash skipped: {}", m.entry);
                false
            }
            (Skip::Target, _) => false,
        });

        // step: display the results.
        medias.iter().for_each(|m| match &m.new_name {
            Some(name) => println!("{} -> {name}", m.entry),
            None => println!("{}", m.entry),
        });

        // step: display summary receipt.
        if !medias.is_empty() || in_place > 0 || clashes > 0 {
            println!();
        }
        println!("total entries: {total}");
        println!("  clashes: {clashes}");
        println!("  in place: {in_place}");
        if medias.is_empty() {
            return Ok(());
        }
        println!("\njoin [by {:?}] to: {target}", self.by);
        if !self.yes {
            utils::prompt_yes_no("apply changes?")?;
        }

        // step: grab the files' parent directories before the consuming operations.
        let dirs = match self.parents {
            true => HashSet::new(),
            false => medias
                .iter()
                .map(|m| m.entry.parent().unwrap())
                .collect::<HashSet<_>>(),
        };

        // step: apply changes, if the user agrees.
        fs::create_dir_all(&target).with_context(|| format!("creating {target:?}"))?;
        match self.by {
            By::Move => medias.rename_move_consuming(),
            By::Copy => medias.copy_consuming(),
        };

        // step: recover from CrossDevice errors.
        if !medias.is_empty() {
            if let By::Move = self.by {
                println!("attempting to fix {} errors", medias.len());
                medias.cross_move_consuming();
            }
        }

        // step: remove the empty parent directories.
        if !self.parents {
            let mut dirs = dirs.into_iter().collect::<Vec<_>>();
            dirs.sort_unstable_by(|m, n| m.cmp(n).reverse());
            dirs.into_iter().for_each(|dir| {
                if let Ok(rd) = fs::read_dir(&dir) {
                    const DS_STORE: &str = ".DS_Store";
                    if rd // .DS_Store might exist on macOS, but should be removed if it is the only file in there.
                        .map(|r| r.is_ok_and(|d| d.file_name() == DS_STORE).then_some(()))
                        .collect::<Option<Vec<_>>>()
                        .is_some_and(|v| !v.is_empty()) // an empty iterator is collected into Some([]).
                    {
                        let dstore = dir.join(DS_STORE);
                        if let Err(err) = fs::remove_file(&dstore) {
                            eprintln!("error: {err}: {dstore:?}");
                        }
                    }
                }
                if let Ok(()) = fs::remove_dir(&dir) {
                    println!("  removed empty dir: {dir}")
                }
            });
        }

        match (medias.is_empty(), self.by) {
            (true, _) => println!("done"),
            (false, By::Move) => println!("still {} errors, giving up", medias.len()),
            (false, By::Copy) => println!("found {} errors", medias.len()),
        }
        Ok(())
    }
}

impl Media {
    fn is_in_place(&self) -> bool {
        let shared = SHARED.get().unwrap();

        let target = &shared.target;
        if shared.force {
            return self.entry.parent().unwrap() == *target;
        }

        match self.entry.is_dir() {
            true => self.entry.starts_with(target),
            false => self.entry.parent().unwrap().starts_with(target),
        }
    }
}

impl_original_entry!(Media);

impl NewEntry for Media {
    fn new_entry(&self) -> Entry {
        let name = self.new_name.as_ref().map(|s| s.as_ref());
        let path = &SHARED.get().unwrap().target;
        path.join(name.unwrap_or_else(|| self.entry().file_name()))
    }
}

impl TryFrom<&Entry> for Media {
    type Error = anyhow::Error;

    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(Media {
            entry: entry.to_owned(),
            new_name: None,
            skip: Skip::No,
        })
    }
}
