mod naming;
mod ops;
mod parts;

use crate::entries::Entry;
pub use naming::*;
pub use ops::*;

pub trait SourceEntry {
    /// The original entry of the file.
    fn src_entry(&self) -> &Entry;
}

pub trait NewEntry {
    /// The new path the file will be renamed to.
    fn new_entry(&self) -> Entry;
}

pub trait NewName {
    fn new_name(&self) -> &str;
    fn collection_parts_new(&self) -> (&str, Option<usize>) {
        parts::collection_parts(self.new_name())
    }
}

pub trait NewNameMut {
    fn new_name_mut(&mut self) -> &mut String;
}

impl<M: NewName + SourceEntry> NewEntry for M {
    fn new_entry(&self) -> Entry {
        self.src_entry().with_file_name(self.new_name())
    }
}

#[macro_export]
macro_rules! impl_source_entry {
    ($t:ty) => {
        impl $crate::medias::SourceEntry for $t {
            fn src_entry(&self) -> &$crate::entries::Entry {
                &self.entry
            }
        }
    };
}

#[macro_export]
macro_rules! impl_new_name {
    ($t:ty) => {
        impl $crate::medias::NewName for $t {
            fn new_name(&self) -> &str {
                &self.new_name
            }
        }
    };
}

#[macro_export]
macro_rules! impl_new_name_mut {
    ($t:ty) => {
        impl $crate::medias::NewNameMut for $t {
            fn new_name_mut(&mut self) -> &mut String {
                &mut self.new_name
            }
        }
    };
}
