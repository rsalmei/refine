mod naming;
mod ops;
mod parts;

use crate::entries::Entry;
pub use naming::*;
pub use ops::*;

pub trait NewName {
    fn new_name(&self) -> &str;
    fn collection_parts_new(&self) -> (&str, Option<usize>) {
        parts::collection_parts(self.new_name())
    }
}

pub trait NewNameMut {
    fn new_name_mut(&mut self) -> &mut String;
}

pub trait OriginalEntry {
    /// The original entry of the file.
    fn entry(&self) -> &Entry;
    fn collection_parts_original(&self) -> (&str, Option<usize>) {
        parts::collection_parts(self.entry().file_name())
    }
}

pub trait NewEntry {
    /// The new path the file will be renamed to.
    fn new_entry(&self) -> Entry;
}

impl<M: NewName + OriginalEntry> NewEntry for M {
    fn new_entry(&self) -> Entry {
        self.entry().with_file_name(self.new_name())
    }
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

#[macro_export]
macro_rules! impl_original_entry {
    ($t:ty) => {
        impl $crate::medias::OriginalEntry for $t {
            fn entry(&self) -> &$crate::entries::Entry {
                &self.entry
            }
        }
    };
}
