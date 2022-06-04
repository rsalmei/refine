use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

const SAMPLE_SIZE: usize = 6 * 1024;

#[derive(PartialOrd, Ord, Eq, PartialEq)]
pub struct Media {
    pub name: String,
    pub size: u64,
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
    pub fn sample(&self) -> io::Result<[u8; SAMPLE_SIZE]> {
        let mut file = File::open(&self.name)?;
        // sample the center of the file.
        let len = file.metadata()?.len();
        file.seek(SeekFrom::Start(len.saturating_sub(SAMPLE_SIZE as u64) / 2))?;
        let mut buf = [0u8; SAMPLE_SIZE];
        file.read(&mut buf)?;
        Ok(buf)
    }
}
