mod media;

use human_repr::HumanRepr;
use media::Media;
use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, fs, iter};

fn main() {
    fn entries(de: DirEntry) -> Box<dyn Iterator<Item = DirEntry>> {
        if de.file_name().to_string_lossy().starts_with(".") {
            return Box::new(iter::empty());
        }
        match de.path().is_dir() {
            true => match fs::read_dir(de.path()) {
                Ok(reader) => Box::new(reader.flatten().map(entries).flatten()),
                Err(err) => {
                    eprintln!("{} -> {err:?}", de.path().display());
                    Box::new(iter::empty())
                }
            },
            false => Box::new(iter::once(de)),
        }
    }

    let mut files = env::args()
        .skip(1)
        .flat_map(|p| match fs::read_dir(&p) {
            Ok(reader) => Some(reader),
            Err(err) => {
                eprintln!("{p} -> {err:?}");
                None
            }
        })
        .flatten()
        .flat_map(|r| entries(r.unwrap()))
        .filter_map(|de| de.metadata().map(|m| (de.path(), m.len())).ok())
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
                    println!("\n{}", size.human_count_bytes());
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
