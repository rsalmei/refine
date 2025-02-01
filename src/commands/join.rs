use super::{EntryKind, Refine};
use crate::impl_original_path;
use crate::utils::{self, kind};
use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Args)]
pub struct Join {
    /// The target directory; will be created if it doesn't exist.
    #[arg(short = 't', long, value_name = "PATH", default_value = ".")]
    target: PathBuf,
    /// The type of join to perform.
    #[arg(short = 'b', long, value_name = "STR", value_enum, default_value_t = By::Move)]
    by: By,
    /// How to resolve clashes.
    #[arg(short = 'c', long, value_name = "STR", value_enum, default_value_t = Clashes::Sequence)]
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
    #[value(aliases = ["seq"])]
    Sequence,
    #[value(aliases = ["pn"])]
    ParentName,
    #[value(aliases = ["np"])]
    NameParent,
    #[value(aliases = ["sk"])]
    Skip,
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
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
    /// Tells whether the target path exists or not.
    target: Result<PathBuf, PathBuf>,
    force: bool,
}

static SHARED: OnceLock<Shared> = OnceLock::new();

impl Refine for Join {
    type Media = Media;
    const OPENING_LINE: &'static str = "Joining files...";
    const ENTRY_KIND: EntryKind = EntryKind::Either;

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        let shared = Shared {
            target: self
                .target
                .canonicalize()
                .map_err(|_| self.target.to_owned()),
            force: self.force,
        };
        SHARED.set(shared).unwrap();
        let total = medias.len();

        // step: read the target directory, which might not be empty, to detect outer clashes (not in medias).
        let mut target_names = Vec::new();
        if let Ok(target) = SHARED.get().unwrap().target.as_ref() {
            if let Ok(in_target) = fs::read_dir(target).map(|rd| rd.flatten()) {
                let in_target = in_target.collect::<Vec<_>>();
                target_names.extend(in_target.iter().flat_map(|e| e.file_name().into_string()));
                medias.extend(in_target.iter().map(|entry| Media {
                    path: entry.path(),
                    new_name: None,
                    skip: Skip::Target,
                }))
            }
        }

        // step: detect clashes (files with the same name in different directories), and resolve them.
        medias.sort_unstable_by(|m, n| {
            // put files already in place first.
            (m.path.file_name(), !m.is_in_place()).cmp(&(n.path.file_name(), !n.is_in_place()))
        });
        let mut clashes = 0;
        medias
            .chunk_by_mut(|m, n| m.path.file_name() == n.path.file_name())
            .filter(|g| g.len() > 1)
            .for_each(|g| {
                clashes += g.len() - 1; // one is (or will be) in target, the others are clashes.
                let path = g[0].path.to_owned();
                let (name, ext) = utils::filename_parts(&path).unwrap(); // files were already checked.
                let dot = if ext.is_empty() { "" } else { "." };
                match self.clashes {
                    Clashes::Sequence => {
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
                        let par = m.path.parent().unwrap_or(Path::new("/"));
                        let par = par.file_name().unwrap().to_str().unwrap();
                        if let Clashes::ParentName = self.clashes {
                            m.new_name = Some(format!("{par}-{name}{dot}{ext}"));
                        } else {
                            m.new_name = Some(format!("{name}-{par}{dot}{ext}"));
                        }
                    }),
                    Clashes::Skip => g.iter_mut().for_each(|m| m.skip = Skip::Yes),
                }
            });

        // step: settle results by removing the files that are in place or skipped.
        medias.sort_unstable_by(|m, n| m.path.cmp(&n.path));
        let mut in_place = 0;
        medias.retain(|m| match (m.skip, m.is_in_place()) {
            (Skip::No, false) => true,
            (Skip::No, true) => {
                in_place += 1;
                println!("already in place: {}{}", m.path.display(), kind(&m.path));
                false
            }
            (Skip::Yes, _) => {
                println!("clash skipped: {}{}", m.path.display(), kind(&m.path));
                false
            }
            (Skip::Target, _) => false,
        });

        // step: display the results.
        medias.iter().for_each(|m| match &m.new_name {
            Some(name) => println!("{}{} -> {name}", m.path.display(), kind(&m.path)),
            None => println!("{}{}", m.path.display(), kind(&m.path)),
        });

        // step: display receipt summary.
        if !medias.is_empty() || in_place > 0 || clashes > 0 {
            println!();
        }
        println!("total files: {total}");
        println!("  clashes: {clashes}");
        println!("  in place: {in_place}");
        if medias.is_empty() {
            return Ok(());
        }
        let target = SHARED.get().unwrap().target.as_ref().unwrap_or_else(|x| x);
        println!("\njoin [by {:?}] to: {}", self.by, target.display());
        if !self.yes {
            utils::prompt_yes_no("apply changes?")?;
        }

        // step: grab the files' parent directories before the consuming operations.
        let dirs = match self.parents {
            true => HashSet::new(),
            false => medias
                .iter()
                .map(|m| m.path.parent().unwrap().to_owned())
                .collect::<HashSet<_>>(),
        };

        // step: apply changes, if the user agrees.
        fs::create_dir_all(target).with_context(|| format!("creating {target:?}"))?;
        match self.by {
            By::Move => utils::rename_move_consuming(&mut medias),
            By::Copy => utils::copy_consuming(&mut medias),
        };

        // step: recover from CrossDevice errors.
        if !medias.is_empty() {
            if let By::Move = self.by {
                println!("attempting to fix {} errors", medias.len());
                utils::cross_move_consuming(&mut medias);
            }
        }

        // step: remove the empty parent directories.
        if !self.parents {
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
                    println!("  removed empty dir: {}", dir.display())
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
        if shared.target.is_err() {
            return false;
        }

        let target = shared.target.as_ref().unwrap();
        if shared.force {
            return self.path.parent().unwrap() == target;
        }

        match self.path.is_dir() {
            true => self.path.starts_with(target),
            false => self.path.parent().unwrap().starts_with(target),
        }
    }
}

impl_original_path!(Media);

impl utils::NewPath for Media {
    fn new_path(&self) -> PathBuf {
        let name = self.new_name.as_ref().map(|s| s.as_ref());
        let path = SHARED.get().unwrap().target.as_ref().unwrap_or_else(|x| x);
        path.join(name.unwrap_or_else(|| self.path.file_name().unwrap()))
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Ok(Media {
            path,
            new_name: None,
            skip: Skip::No,
        })
    }
}
