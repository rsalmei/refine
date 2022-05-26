use std::collections::HashMap;
use std::fs::{DirEntry, File};
use std::io::Read;
use std::path::PathBuf;
use std::{env, fs, io, iter};

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
                // count += 1;
                // println!("\n{}", size.human_repr());
                // acc.sort_unstable();
                // acc.iter().for_each(|m| println!("{}", m.name));
                let mut split = HashMap::with_capacity(acc.len());
                acc.iter().for_each(|&m| {
                    split
                        .entry(m.sample().unwrap())
                        .or_insert_with(|| vec![])
                        .push(m)
                });
                split.values_mut().filter(|v| v.len() > 1).for_each(|v| {
                    count += 1;
                    println!("\n{}", size.human_repr());
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

const SAMPLE_SIZE: usize = 1024;

#[derive(PartialOrd, Ord, Eq, PartialEq)]
struct Media {
    name: String,
    size: u64,
}

impl From<(PathBuf, u64)> for Media {
    fn from((path, size): (PathBuf, u64)) -> Self {
        Media {
            name: path.to_string_lossy().into_owned(),
            size,
        }
    }
}

impl Media {
    fn sample(&self) -> io::Result<[u8; SAMPLE_SIZE]> {
        let mut file = File::open(&self.name)?;
        let mut buf = [0u8; SAMPLE_SIZE];
        let _ = file.read(&mut buf);
        Ok(buf)
    }
}

trait HumanRepr {
    fn human_repr(self) -> String;
}

impl HumanRepr for u64 {
    fn human_repr(self) -> String {
        const SPEC: &[&str] = &["", "K", "M", "G", "T", "P", "E", "Z", "Y"];
        let mut value = self as f64;
        for scale in SPEC {
            match value {
                // _ if value < 9.995 => return format!("{value:1.2} {scale}B"),
                // _ if value < 99.95 => return format!("{value:2.1} {scale}B"),
                // _ if value < 999.5 => return format!("{value:3.0} {scale}B"),
                _ if value < 999.95 => return format!("{value:3.1} {scale}B"),
                _ => value /= 1000.,
            }
        }

        format!("{value:3} +B")
    }
}
