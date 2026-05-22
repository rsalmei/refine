use crate::entries::Entry;
use std::fmt::Display;
use std::{fmt, path};
use yansi::{Paint, Style};

/// A [Display] implementation for [Entry] that prints its full path with the name first, then the
/// parent directory.
#[derive(Debug)]
pub struct DisplayInvertedPath<'a>(pub &'a Entry);

/// A [Display] implementation for [Entry] that prints only the filename, without the parent directory.
#[derive(Debug)]
pub struct DisplayFilename<'a>(pub &'a Entry);

const DIR_STYLE: (Style, Style) = {
    let parent_dir = Style::new().yellow();
    (parent_dir, parent_dir.bold())
};
const FILE_STYLE: (Style, Style) = {
    let parent_file = Style::new().cyan();
    (parent_file, parent_file.bold())
};

/// A [Display] implementation for [Entry] that prints the usual full path, with the parent
/// directory first, then the name.
impl Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (parent, name, symbol) = display_parts(self);
        let (p_style, n_style) = if self.is_dir { DIR_STYLE } else { FILE_STYLE };
        write!(
            f,
            "{}{}{}",
            parent.paint(p_style),
            name.paint(n_style),
            symbol.paint(n_style)
        )
    }
}

impl Display for DisplayInvertedPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entry = self.0;
        let (parent, name, symbol) = display_parts(entry);
        let (p_style, n_style) = if entry.is_dir { DIR_STYLE } else { FILE_STYLE };
        write!(
            f,
            "{}{} {}",
            name.paint(n_style),
            symbol.paint(n_style),
            parent.paint(p_style)
        )
    }
}

impl Display for DisplayFilename<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entry = self.0;
        let (_, name, symbol) = display_parts(entry);
        let (_, style) = if entry.is_dir { DIR_STYLE } else { FILE_STYLE };
        write!(f, "{}{}", name.paint(style), symbol.paint(style))
    }
}

/// Get the parent directory and name for an entry, including the trailing slash if needed.
///
/// They are used by [DisplayInvertedPath] and [DisplayFilename] implementations, which style them.
fn display_parts(entry: &Entry) -> (&str, &str, &'static str) {
    let full = entry.to_str();
    let pos = full.rfind(path::MAIN_SEPARATOR).map_or(0, |p| p + 1);
    let (parent, name) = full.split_at(pos);
    let symbol = match entry.is_dir {
        true => path::MAIN_SEPARATOR_STR,
        false => "",
    };
    (parent, name, symbol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fn_display_parts() {
        #[track_caller]
        fn case(p: impl Into<PathBuf>, is_dir: bool, expected: (&str, &str, &str)) {
            let path = p.into();
            let entry = Entry { path, is_dir };
            assert_eq!(expected, display_parts(&entry));
        }

        // directory cases.
        case("foo/dir", true, ("foo/", "dir", "/"));
        case("/foo/dir", true, ("/foo/", "dir", "/"));
        case("foo/dir/", true, ("foo/", "dir", "/"));
        case("/foo/dir/", true, ("/foo/", "dir", "/"));
        case("x/y/", true, ("x/", "y", "/"));
        case("/x/y/", true, ("/x/", "y", "/"));
        case(".", true, ("", ".", "/"));
        case("..", true, ("", "..", "/"));
        case("/", true, ("", "", "/"));
        case("./", true, ("", ".", "/"));
        case("../", true, ("", "..", "/"));
        case("dir", true, ("", "dir", "/"));
        case("dir/", true, ("", "dir", "/"));
        case("./dir", true, ("./", "dir", "/"));
        case("./dir/", true, ("./", "dir", "/"));

        // file cases.
        case("file.txt", false, ("", "file.txt", ""));
        case("./file.txt", false, ("./", "file.txt", ""));
        case("dir/file.txt", false, ("dir/", "file.txt", ""));
        case("./dir/file.txt", false, ("./dir/", "file.txt", ""));
        case(".hidden", false, ("", ".hidden", ""));
        case("./dir/.hidden", false, ("./dir/", ".hidden", ""));
    }
}
