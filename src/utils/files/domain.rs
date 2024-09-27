use std::path::{Path, PathBuf};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Sequence {
    pub len: usize,
    pub seq: u32,
}

pub trait NewName {
    fn new_name(&self) -> &str;
}

pub trait NewNameMut {
    fn new_name_mut(&mut self) -> &mut String;
}

pub trait OriginalPath {
    /// The original path to the file.
    fn path(&self) -> &Path;
}

pub trait NewPath {
    /// The original path to the file.
    fn new_path(&self) -> PathBuf;
}

impl<M: NewName + OriginalPath> NewPath for M {
    fn new_path(&self) -> PathBuf {
        self.path().with_file_name(self.new_name())
    }
}

#[macro_export]
macro_rules! impl_new_name {
    ($t:ty) => {
        impl $crate::utils::NewName for $t {
            fn new_name(&self) -> &str {
                &self.new_name
            }
        }
        impl $crate::utils::NewNameMut for $t {
            fn new_name_mut(&mut self) -> &mut String {
                &mut self.new_name
            }
        }
    };
}

#[macro_export]
macro_rules! impl_original_path {
    ($t:ty) => {
        impl $crate::utils::OriginalPath for $t {
            fn path(&self) -> &std::path::Path {
                &self.path
            }
        }
    };
}