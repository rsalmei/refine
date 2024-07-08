use anyhow::{anyhow, Result};
use regex::Regex;
use std::io;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::{atomic, Arc, OnceLock};

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
pub fn prompt_yes_no(msg: &str) -> Result<()> {
    let mut input = String::new();
    loop {
        user_aborted()?;
        print!("{msg} [y|n]: ");
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        match input.trim() {
            _ if !is_running() => continue, // never return Ok or cancelled if the user has aborted.
            "y" => break Ok(()),
            "n" => break Err(anyhow!("cancelled")),
            _ => {}
        }
    }
}

/// The running flag, used to check if the user aborted.
pub fn running_flag() -> &'static Arc<AtomicBool> {
    static RUNNING: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    RUNNING.get_or_init(|| Arc::new(AtomicBool::new(true)))
}

/// Check whether the program should continue running.
pub fn is_running() -> bool {
    running_flag().load(atomic::Ordering::Relaxed)
}

/// Check whether the user asked to abort. It's the same as `!running()`, but return a Result.
pub fn user_aborted() -> Result<()> {
    match is_running() {
        true => Ok(()),
        false => Err(anyhow!("aborted")),
    }
}

/// Return a static string, suitable for displaying, regarding the state of some computation
/// that might have been aborted.
pub fn aborted(cond: bool) -> &'static str {
    (cond && !is_running())
        .then_some(" (partial, aborted)")
        .unwrap_or_default()
}
