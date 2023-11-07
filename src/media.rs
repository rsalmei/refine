use std::cmp::Ordering;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

const SAMPLE_SIZE: usize = 2 * 1024;

#[derive(Debug)]
pub struct Media {
    pub path: PathBuf,
    pub size: u64,
    pub words: Vec<String>,
}

impl Media {
    pub fn new((path, size): (PathBuf, u64)) -> Self {
        let mut words = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .split(&[' ', '.', '-', '_'])
            .filter(|&s| !s.is_empty())
            .filter(|&s| !(s.len() == 1 && s.bytes().next().unwrap().is_ascii_alphabetic()))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        words.sort_unstable();
        // let words = words.into_iter().collect();

        Media { path, size, words }
    }

    pub fn sample(&self) -> io::Result<[u8; SAMPLE_SIZE]> {
        let mut file = File::open(&self.path)?;
        // sample the center of the file.
        let len = file.metadata()?.len();
        file.seek(SeekFrom::Start(len.saturating_sub(SAMPLE_SIZE as u64) / 2))?;
        let mut buf = [0u8; SAMPLE_SIZE];
        let _ = file.read(&mut buf)?;
        Ok(buf)
    }
}

impl PartialEq for Media {
    fn eq(&self, other: &Self) -> bool {
        self.path.eq(&other.path)
    }
}

impl Eq for Media {}

impl PartialOrd for Media {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Media {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}
