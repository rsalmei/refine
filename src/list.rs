use anyhow::{anyhow, Result};
use clap::{Args, ValueEnum};
use human_repr::HumanCount;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct List {
    #[arg(short, long, value_enum, default_value_t = By::Name)]
    by: By,
    #[arg(short, long)]
    desc: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum By {
    Name,
    Size,
    Path,
}

fn opt() -> &'static List {
    match &super::args().cmd {
        super::Command::List(opt) => opt,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    name: String,
    size: u64,
}

pub fn list(mut medias: Vec<Media>) -> Result<()> {
    println!("Listing files...");
    let desc = if opt().desc { " (desc)" } else { "" };
    println!("  - by: {:?}{}", opt().by, desc);
    println!();

    let compare = match opt().by {
        By::Name => |a: &Media, b: &Media| a.name.cmp(&b.name),
        By::Size => |a: &Media, b: &Media| a.size.cmp(&b.size),
        By::Path => |a: &Media, b: &Media| a.path.cmp(&b.path),
    };
    let compare: &dyn Fn(&Media, &Media) -> Ordering = match opt().desc {
        true => &|a, b| compare(a, b).reverse(),
        false => &compare,
    };
    medias.sort_unstable_by(compare);
    medias
        .iter()
        .for_each(|m| println!("{} - {}", m.size.human_count_bytes(), m.path.display()));

    if !medias.is_empty() {
        println!();
    }
    let size = medias.iter().map(|m| m.size).sum::<u64>();
    println!("total files: {} ({})", medias.len(), size.human_count("B"));
    Ok(())
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let name = path
            .file_stem()
            .ok_or_else(|| anyhow!("no file name: {path:?}"))?
            .to_str()
            .ok_or_else(|| anyhow!("file name str: {path:?}"))?;
        Ok(Self {
            name: name.to_lowercase(),
            size: fs::metadata(&path)?.len(),
            path,
        })
    }
}
