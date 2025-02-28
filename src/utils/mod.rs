mod running;

use anyhow::{Result, anyhow};
pub use running::*;
use std::collections::HashSet;
use std::error::Error;
use std::io::{Write, stdin, stdout};
use std::str::FromStr;
use std::sync::{LazyLock, Mutex, mpsc};
use std::thread;
use std::time::Duration;

/// Prompt the user for confirmation.
pub fn prompt_yes_no(msg: impl Into<Box<str>>) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let msg = msg.into(); // I need ownership of an immutable message here.
    let fun = move |input: &mut String| {
        aborted()?;
        print!("{msg} [y|n|q]: ");
        stdout().flush()?;
        input.clear();
        stdin().read_line(input)?;
        Ok(())
    };
    thread::spawn(move || {
        let mut input = String::new();
        let res = loop {
            match (fun(&mut input), input.trim()) {
                (Err(err), _) => break Err(err),
                (Ok(()), "y") => break Ok(()),
                (Ok(()), "n" | "q") => break Err(anyhow!("cancelled")),
                _ => {}
            }
        };
        let _ = tx.send(res);
    });

    loop {
        match rx.recv_timeout(Duration::from_millis(1000 / 2)) {
            Ok(res) => break res,
            Err(_) => aborted()?,
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
        .ok_or_else(|| anyhow!("invalid key=value: {s:?}"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
