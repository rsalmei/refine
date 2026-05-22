mod natural;
mod prompt;
mod running;

use anyhow::{Result, anyhow};
use std::error::Error;
use std::str::FromStr;

pub use natural::*;
pub use prompt::*;
pub use running::*;

/// Parse a key-value pair from a string, for use in clap.
pub fn parse_key_value<K, V>(s: &str) -> Result<(K, V)>
where
    K: FromStr<Err: Error + Send + Sync + 'static>,
    V: FromStr<Err: Error + Send + Sync + 'static>,
{
    let pos = s
        .find('=')
        .ok_or_else(|| anyhow!("missing =value in: {s:?}"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        use yansi::{Color, Paint};
        eprintln!("{}", format_args!($($arg)*).paint(Color::Green).bold());
    }};
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {{
        use yansi::{Color, Paint};
        eprintln!("{}: {}", "warning".paint(Color::Yellow).bold(), format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        use yansi::{Color, Paint};
        eprintln!("{}: {}", "error".paint(Color::Red).bold(), format_args!($($arg)*));
    }};
}
