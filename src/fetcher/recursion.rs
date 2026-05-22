use clap::Args;

#[derive(Debug, Args)]
pub struct RecursionArgs {
    /// The maximum recursion depth; use 0 for unlimited, 1 for shallow, and n for depth n.
    #[arg(short = 'R', long, default_value_t = 0, value_name = "INT", help_heading = Some("Fetch"))]
    recursion: usize,
}

/// Recursion mode for fetching entries.
///
/// Full means to fetch all descendants, shallow means to fetch only direct children, and depth(n)
/// means to fetch up to n levels of descendants.
#[derive(Debug, Copy, Clone)]
pub enum Recursion {
    Full,
    Shallow,
    Depth(usize),
}

impl From<RecursionArgs> for Recursion {
    fn from(args: RecursionArgs) -> Self {
        match args.recursion {
            0 => Recursion::Full,
            1 => Recursion::Shallow,
            d => Recursion::Depth(d - 1),
        }
    }
}

impl Recursion {
    pub fn deeper(self) -> Option<Self> {
        match self {
            Recursion::Full => Some(Recursion::Full),
            Recursion::Shallow => None,
            Recursion::Depth(d) => d.checked_sub(1).map(Recursion::Depth),
        }
    }
}
