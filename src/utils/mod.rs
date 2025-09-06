mod natural;
mod running;

use anyhow::{Result, anyhow};
pub use natural::*;
pub use running::*;
use std::collections::HashSet;
use std::error::Error;
use std::io::{Write, stdin, stdout};
use std::str::FromStr;
use std::sync::{LazyLock, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub enum PromptError {
    No,
    Quit,
}

impl From<anyhow::Error> for PromptError {
    fn from(_: anyhow::Error) -> Self {
        PromptError::Quit
    }
}

impl From<PromptError> for anyhow::Error {
    fn from(err: PromptError) -> Self {
        match err {
            PromptError::No => anyhow!("declined"),
            PromptError::Quit => anyhow!("cancelled"),
        }
    }
}

/// Prompt the user for confirmation.
pub fn prompt_yes_no(msg: impl Into<Box<str>>) -> Result<(), PromptError> {
    let (tx, rx) = mpsc::channel();
    let msg = msg.into(); // I need ownership of an immutable message here.
    let f = move |input: &mut String| {
        aborted()?;
        print!("{msg} [y|n|q]: ");
        stdout().flush()?;
        input.clear();
        stdin().read_line(input)?;
        Ok::<_, anyhow::Error>(())
    };
    thread::spawn(move || {
        let mut input = String::new();
        let res = loop {
            match (f(&mut input), input.trim()) {
                (Err(err), _) => break Err(err.into()),
                (Ok(()), "y" | "yes") => break Ok(()),
                (Ok(()), "n" | "no") => break Err(PromptError::No),
                (Ok(()), "q" | "quit") => break Err(PromptError::Quit),
                _ => {}
            }
        };
        let _ = tx.send(res);
    });

    loop {
        match rx.recv_timeout(Duration::from_millis(1000 / 2)) {
            Ok(res) => break res,
            Err(_) => aborted().map_err(|_| PromptError::Quit)?,
        }
    }
}

/// Intern a string, to prevent duplicates and redundant allocations.
pub fn intern(text: &str) -> &'static str {
    static CACHE: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);

    let mut cache = CACHE.lock().unwrap();
    match cache.get(text) {
        Some(x) => x,
        None => {
            let interned = Box::leak(text.to_owned().into_boxed_str());
            cache.insert(interned);
            interned
        }
    }
}

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
