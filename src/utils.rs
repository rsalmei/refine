use anyhow::anyhow;
use regex::Regex;
use std::io;
use std::io::Write;
use std::sync::OnceLock;

/// Strip sequence numbers from a filename.
pub fn strip_sequence(name: &str) -> &str {
    static RE_MULTI_MACOS: OnceLock<Regex> = OnceLock::new();
    static RE_MULTI_LOCAL: OnceLock<Regex> = OnceLock::new();
    let rem = RE_MULTI_MACOS.get_or_init(|| Regex::new(r" copy( \d+)?$").unwrap());
    let rel = RE_MULTI_LOCAL.get_or_init(|| Regex::new(r"-\d+$").unwrap());

    // replace_all() would allocate a new string, which would be a waste.
    let name = rem.split(name).next().unwrap(); // even if the name is " copy", this returns an empty str.
    rel.split(name).next().unwrap() // same as above, even if the name is "-1", this returns an empty str.
}

/// Prompt the user for confirmation.
pub fn prompt_yes_no(msg: &str) -> anyhow::Result<()> {
    let mut input = String::new();
    loop {
        print!("{msg} [y|n]: ");
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        match input.trim() {
            "y" => break Ok(()),
            "n" => break Err(anyhow!("cancelled")),
            _ => {}
        }
    }
}
