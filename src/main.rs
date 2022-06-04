mod media;

use human_repr::HumanRepr;
use media::Media;
use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, fs, iter};

fn main() {
    fn entries(pb: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
        if pb.file_name().unwrap().to_string_lossy().starts_with(".") {
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

    let mut files = env::args()
        .skip(1)
        .map(|s| s.parse::<PathBuf>().unwrap())
        .flat_map(entries)
        .filter_map(|pb| pb.metadata().map(|m| (pb, m.len())).ok())
        .map(Media::from)
        .collect::<Vec<_>>();
    files.sort_unstable_by_key(|m| m.size);

    let (mut count, mut size, mut acc) = (0, 0, vec![]);
    files.iter().map(Some).chain(None).for_each(|om| {
        match om {
            Some(m) if m.size == size => return acc.push(m),
            _ if acc.len() < 2 => {}
            _ => {
                let mut split = HashMap::with_capacity(acc.len());
                acc.iter().for_each(|&m| {
                    split
                        .entry(m.sample().unwrap())
                        .or_insert_with(|| vec![])
                        .push(m)
                });
                split.values_mut().filter(|v| v.len() > 1).for_each(|v| {
                    count += 1;
                    println!("\n{} x{}", size.human_count_bytes(), v.len());
                    v.sort_unstable();
                    v.iter().for_each(|m| println!("{}", m.name));
                })
            }
        }
        if let Some(m) = om {
            acc.clear();
            acc.push(m);
            size = m.size;
        }
    });

    println!("\ntotal files: {} ({count} duplicates)", files.len());
}
