mod naming;
mod ops;

use crate::entries::Entry;
pub use naming::*;
pub use ops::*;

pub trait NewName {
    fn new_name(&self) -> &str;
}

pub trait NewNameMut {
    fn new_name_mut(&mut self) -> &mut String;
}

pub trait OriginalEntry {
    /// The original entry of the file.
    fn entry(&self) -> &Entry;
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
        impl $crate::media::NewName for $t {
            fn new_name(&self) -> &str {
                &self.new_name
            }
        }
    };
}

#[macro_export]
macro_rules! impl_new_name_mut {
    ($t:ty) => {
        impl $crate::media::NewNameMut for $t {
            fn new_name_mut(&mut self) -> &mut String {
                &mut self.new_name
            }
        }
    };
}

#[macro_export]
macro_rules! impl_original_entry {
    ($t:ty) => {
        impl $crate::media::OriginalEntry for $t {
            fn entry(&self) -> &$crate::entries::Entry {
                &self.entry
            }
        }
    };
}
