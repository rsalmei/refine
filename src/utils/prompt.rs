use super::aborted;
use anyhow::{Result, anyhow};
use std::io::{Write, stdin, stdout};
use std::sync::mpsc;
use std::thread;

#[derive(Debug)]
pub enum PromptResponse {
    Yes,
    No,
}

impl PromptResponse {
    /// Verify that the result was "yes", returning an error otherwise.
    pub fn yes(self) -> Result<()> {
        match self {
            PromptResponse::Yes => Ok(()),
            PromptResponse::No => Err(anyhow!("refused by user")),
        }
    }
}

/// Prompt the user for applying changes, returning an error if the response was not "yes".
pub fn prompt_apply_changes() -> Result<()> {
    prompt_user("Apply changes?", false)?.yes()
}

/// Prompt the user for confirmation, returning an error if the response was not "yes".
pub fn prompt_yes_no(msg: impl Into<Box<str>>) -> Result<()> {
    prompt_user(msg, false)?.yes()
}

/// Prompt the user for confirmation, returning the response as a [PromptResponse].
pub fn prompt_yes_no_halt(msg: impl Into<Box<str>>) -> Result<PromptResponse> {
    prompt_user(msg, true)
}

fn prompt_user(msg: impl Into<Box<str>>, stop: bool) -> Result<PromptResponse> {
    let msg = msg.into(); // I need ownership of an immutable message here.
    let read_chars = move |input: &mut String| {
        aborted()?;
        let options = match stop {
            false => "[yes/no]",
            true => "[yes/no/stop]",
        };
        print!("{msg} {options}: ");
        stdout().flush()?;
        input.clear();
        stdin().read_line(input)?;
        Ok::<(), anyhow::Error>(())
    };

    // allow interrupting the prompt with Ctrl+C, since there's a ctrl-c handler installed.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut input = String::new();
        let res = loop {
            match (read_chars(&mut input), input.trim()) {
                (Ok(()), "y" | "yes") => break Ok(PromptResponse::Yes),
                (Ok(()), "n" | "no") => break Ok(PromptResponse::No),
                (Ok(()), "s" | "stop") if stop => break Err(anyhow!("stopped by user")),
                (Err(err), _) => break Err(err),
                _ => {}
            }
        };
        let _ = tx.send(res);
    });

    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(1000 / 3)) {
            Ok(res) => break res,
            Err(mpsc::RecvTimeoutError::Timeout) => aborted()?,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err(anyhow!("prompt thread disconnected"));
            }
        }
    }
}
