use crate::entries::EntryKind;
use crate::{options, utils};
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
    to: PathBuf,
    /// The strategy to use when joining files.
    #[arg(short = 's', long, value_enum, default_value_t = Strategy::Move)]
    strategy: Strategy,
    /// Force joining already in place files and directories, i.e., in subdirectories of the target.
    #[arg(short = 'f', long)]
    force: bool,
    /// Do not remove the empty parent directories after joining.
    #[arg(short = 'n', long)]
    no_remove: bool,
    /// Skip the confirmation prompt, useful for automation.
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Strategy {
    Move,
    Copy,
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    new_name: Option<String>,
}

static TARGET: OnceLock<Result<PathBuf, PathBuf>> = OnceLock::new();

options!(Join => EntryKind::Either);

pub fn run(mut medias: Vec<Media>) -> Result<()> {
    println!("=> Joining files...\n");
    TARGET
        .set(opt().to.canonicalize().map_err(|_| opt().to.to_owned()))
        .unwrap();
    let kind = |p: &Path| if p.is_dir() { "/" } else { "" };

    // step: detect clashes, files with the same name in different directories, and apply a sequential number.
    medias.sort_unstable_by(|m, n| {
        (m.path.file_name(), !m.is_in_place()).cmp(&(n.path.file_name(), !n.is_in_place()))
    });
    let mut clashes = 0;
    medias
        .chunk_by_mut(|m, n| m.path.file_name() == n.path.file_name())
        .filter(|g| g.len() > 1)
        .for_each(|g| {
            clashes += g.len() - 1;
            let path = g[0].path.to_owned();
            let (name, ext) = utils::filename_parts(&path).unwrap(); // files were already checked.
            (1..).zip(g).skip(1).for_each(|(i, m)| {
                let dot = if ext.is_empty() { "" } else { "." };
                m.new_name = format!("{name}-{i}{dot}{ext}").into();
            })
        });

    // step: detect already in-place files.
    medias.sort_unstable_by(|m, n| m.path.cmp(&n.path));
    let mut in_place = 0;
    medias.retain(|m| match m.is_in_place() {
        false => true,
        true => {
            in_place += 1;
            println!("already in place: {}{}", m.path.display(), kind(&m.path));
            false
        }
    });

    // step: display the results.
    medias.iter().for_each(|m| match &m.new_name {
        Some(name) => println!("{}{} -> {name}", m.path.display(), kind(&m.path)),
        None => println!("{}{}", m.path.display(), kind(&m.path)),
    });

    // step: display receipt summary.
    if !medias.is_empty() || in_place > 0 {
        println!();
    }
    println!("total files: {}", medias.len() + in_place);
    println!("  in place: {in_place}");
    println!("  clashes: {clashes}");
    if medias.is_empty() {
        return Ok(());
    }
    let target = TARGET.get().unwrap().as_ref().unwrap_or_else(|x| x);
    println!("\njoin [by {:?}] to: {}", opt().strategy, target.display());
    if !opt().yes {
        utils::prompt_yes_no("apply changes?")?;
    }

    // step: grab the files' parent directories before the consuming operations.
    let dirs = match opt().no_remove {
        true => HashSet::new(),
        false => medias
            .iter()
            .map(|m| m.path.parent().unwrap().to_owned())
            .collect::<HashSet<_>>(),
    };

    // step: apply changes, if the user agrees.
    fs::create_dir_all(target).with_context(|| format!("creating {target:?}"))?;
    match opt().strategy {
        Strategy::Move => utils::rename_move_consuming(&mut medias),
        Strategy::Copy => utils::copy_consuming(&mut medias),
    };

    // step: recover from CrossDevice errors.
    if !medias.is_empty() {
        if let Strategy::Move = opt().strategy {
            println!("attempting to fix {} errors", medias.len());
            utils::cross_move_consuming(&mut medias);
        }
    }

    // step: remove the empty parent directories.
    if !opt().no_remove {
        dirs.into_iter().for_each(|dir| {
            if let Ok(rd) = fs::read_dir(&dir) {
                if rd // .DS_Store might exist on macOS, but should be removed if it is the only file in there.
                    .map(|r| r.is_ok_and(|d| d.file_name() == ".DS_Store").then_some(()))
                    .collect::<Option<Vec<_>>>()
                    .is_some_and(|v| !v.is_empty())
                {
                    let dstore = dir.join(".DS_Store");
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

    match (medias.is_empty(), opt().strategy) {
        (true, _) => println!("done"),
        (false, Strategy::Move) => println!("still {} errors, giving up", medias.len()),
        (false, Strategy::Copy) => println!("found {} errors", medias.len()),
    }
    Ok(())
}

impl Media {
    fn is_in_place(&self) -> bool {
        if TARGET.get().unwrap().is_err() {
            return false;
        }

        let target = TARGET.get().unwrap().as_ref().unwrap();
        if opt().force {
            return self.path.parent().unwrap() == target;
        }

        match self.path.is_dir() {
            true => self.path.starts_with(target),
            false => self.path.parent().unwrap().starts_with(target),
        }
    }
}

impl utils::OriginalPath for Media {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl utils::NewPath for Media {
    fn new_path(&self) -> PathBuf {
        let name = self.new_name.as_ref().map(|s| s.as_ref());
        let path = TARGET.get().unwrap().as_ref().unwrap_or_else(|x| x);
        path.join(name.unwrap_or_else(|| self.path.file_name().unwrap()))
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Ok(Media {
            path,
            new_name: None,
        })
    }
}
