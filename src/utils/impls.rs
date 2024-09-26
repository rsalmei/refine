#[macro_export]
macro_rules! impl_new_name {
    ($t:ty) => {
        impl $crate::utils::NewName for $t {
            fn new_name(&self) -> &str {
                &self.new_name
            }
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

