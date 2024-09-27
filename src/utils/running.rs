use anyhow::{anyhow, Result};
use std::sync::atomic::AtomicBool;
use std::sync::{atomic, LazyLock};

static RUNNING_FLAG: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(true));

/// Check whether the program should continue running.
pub fn is_running() -> bool {
    RUNNING_FLAG.load(atomic::Ordering::Relaxed)
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

/// Install a Ctrl-C handler. It must be called only once.
pub fn install_ctrl_c_handler() {
    if let Err(err) = ctrlc::set_handler(move || {
        eprintln!("aborting...");
        RUNNING_FLAG.store(false, atomic::Ordering::Relaxed);
    }) {
        eprintln!("error: set Ctrl-C handler: {err:?}");
    }
}
