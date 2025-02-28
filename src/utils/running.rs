use anyhow::{Result, anyhow};
use std::fmt::{Display, Formatter};
use std::sync::atomic::AtomicBool;
use std::sync::{LazyLock, atomic};

static RUNNING_FLAG: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(true));

/// Check whether the program should continue running.
pub fn is_running() -> bool {
    RUNNING_FLAG.load(atomic::Ordering::Relaxed)
}

/// Check whether the user asked to abort, and if so, return an error which can be propagated.
pub fn aborted() -> Result<()> {
    match is_running() {
        true => Ok(()),
        false => Err(anyhow!("user asked to abort")),
    }
}

/// Return an object that prints an abort marker if the program is aborted.
pub fn display_abort(cond: bool) -> impl Display {
    DisplayAbort { cond }
}

#[derive(Debug, Copy, Clone)]
pub struct DisplayAbort {
    cond: bool,
}

impl Display for DisplayAbort {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.cond && !is_running() {
            write!(f, " (aborted)")?;
        }
        Ok(())
    }
}

/// Install a Ctrl-C handler. It must be called only once.
pub fn install_ctrl_c_handler() {
    let handler = || {
        eprintln!(" aborting...");
        RUNNING_FLAG.store(false, atomic::Ordering::Relaxed);
    };
    if let Err(err) = ctrlc::set_handler(handler) {
        eprintln!("error: set Ctrl-C handler: {err:?}");
    }
}
