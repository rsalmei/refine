use crate::commands::Refine;
use crate::entries::{Entry, EntrySet, Warnings};
use crate::utils::{self, display_abort};
use Verdict::*;
use anyhow::{Context, Result, anyhow};
use clap::{Args, ValueEnum};
use regex::Regex;
use std::fmt::Display;
use std::io::{Write, stdout};
use std::time::Duration;
use ureq::Agent;
use ureq::http::StatusCode;

#[derive(Debug, Args)]
pub struct Probe {
    /// Pick a subset of the files to probe.
    #[arg(short = 'p', long, value_name = "REGEX")]
    pick: Option<String>,
    /// The URL to probe filenames against (use `$` as placeholder, e.g. https://example.com/$/).
    #[arg(short = 'u', long)]
    url: String,
    /// The HTTP connection and read timeouts in milliseconds.
    #[arg(short = 't', long, default_value_t = 2000, value_name = "INT")]
    timeout: u64,
    /// The initial time to wait between retries in milliseconds.
    #[arg(short = 'n', long, default_value_t = 1000, value_name = "INT")]
    min_wait: u64,
    /// The factor by which to increase the time to wait between retries.
    #[arg(short = 'b', long, default_value_t = 1.5, value_name = "FLOAT")]
    backoff: f64,
    /// The maximum time to wait between retries in milliseconds.
    #[arg(short = 'a', long, default_value_t = 5000, value_name = "INT")]
    max_wait: u64,
    /// The maximum number of retries; use 0 to disable and -1 to retry indefinitely.
    #[arg(short = 'r', long, default_value_t = -1, value_name = "INT")]
    retries: i32,
    /// Specify when to display errors.
    #[arg(short = 'e', long, default_value_t = Errors::Each10, value_name = "STR", value_enum)]
    errors: Errors,
    // /// The HTTP request method to use.
    // #[arg(short = 'm', long, default_value = "HEAD", value_name = "STR")]
    // method: Method,
    // /// The number of concurrent connections.
    // #[arg(short = 'c', long, default_value = "10", value_name = "INT")]
    // connections: u8,
    // /// The rate limit in requests per second.
    // #[arg(short = 'r', long, default_value = "10", value_name = "INT")]
    // rate: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
pub enum Errors {
    #[value(alias = "n")]
    Never,
    #[value(alias = "l")]
    Last,
    #[value(alias = "a")]
    Always,
    #[value(aliases = ["e", "10"])]
    Each10,
}

#[derive(Debug)]
pub struct Media {
    name: String,
    verdict: Verdict,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Verdict {
    Pending,
    Valid,
    Invalid,
    Failed,
}

impl Refine for Probe {
    type Media = Media;
    const OPENING_LINE: &'static str = "Checking files online...";
    const HANDLES: EntrySet = EntrySet::Files;

        // make sure the URL contains a single `$` placeholder.
        if self.url.bytes().filter(|&b| b == b'$').count() != 1 {
            return Err(anyhow!("URL must contain a single `$` placeholder"))
                .with_context(|| format!("invalid URL: {:?}", self.url));
        }

        // make sure the URL is valid, but parsing it as a URI always succeeds.
        // it seems the only way to check it is by actually sending a request.
        ureq::head(&self.url)
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .with_context(|| format!("invalid URL: {:?}", self.url))?;

    fn tweak(&mut self, _: &Warnings) {
        if self.retries < 0 && self.errors == Errors::Last {
            println!(
                "Can't show \"last\" error display for indefinite retries, switching to \"never\".\n"
            );
            self.errors = Errors::Never;
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: keep only unique file names (sequences were already removed).
        medias.sort_unstable_by(|m, n| m.name.cmp(&n.name));
        medias.dedup_by(|m, n| m.name == n.name);

        // step: pick a subset of the files to probe.
        match &self.pick {
            Some(s) => {
                let re = Regex::new(s).context("invalid regex")?;
                medias.retain(|m| re.is_match(&m.name));
                println!("probing names matching {s:?}: {}", medias.len());
            }
            None => {
                println!("probing all names: {}", medias.len());
            }
        }

        let total_names = medias.len();

        // step: probe each file name.
        let client = Agent::config_builder()
            .timeout_global(Some(Duration::from_millis(self.timeout)))
            .http_status_as_error(false)
            .build()
            .into();
        for media in &mut *medias {
            print!("  {}: ", media.name);
            stdout().flush()?;
            media.verdict = match self.probe_one(&media.name, &client) {
                Ok(verdict) => verdict,
                Err(_) => break,
            };
        }

        // step: display the results.
        let valid = medias.iter().filter(|m| m.verdict == Valid).count();
        let failed = medias.iter().filter(|m| m.verdict == Failed).count();
        let pending = medias.iter().filter(|m| m.verdict == Pending).count();
        medias.retain(|m| m.verdict == Invalid);
        if !medias.is_empty() {
            println!("\ninvalid names:");
            medias.iter().for_each(|m| println!("  {}", m.name));
        }

        // step: display receipt summary.
        println!("\ntotal names: {total_names}");
        println!("  valid  : {valid}");
        println!("  invalid: {}", medias.len());
        if failed > 0 {
            println!("  failed : {failed}");
        }
        if pending > 0 {
            println!("  pending: {pending}{}", display_abort(true));
        }

        Ok(())
    }
}

impl Probe {
    fn probe_one(&self, name: &str, client: &Agent) -> Result<Verdict> {
        let url = self.url.replace("$", name);
        let (mut wait, mut spaces, mut retry) = (self.min_wait, 0, 0);
        let verdict = loop {
            utils::aborted()?;
            let (full, brief): (&dyn Display, _) = match client.head(&url).call() {
                Ok(resp) => match resp.status() {
                    StatusCode::OK | StatusCode::FORBIDDEN => break Valid,
                    StatusCode::NOT_FOUND => break Invalid,
                    StatusCode::TOO_MANY_REQUESTS => (&"too many requests", "."),
                    _ => (&resp.status().to_string(), "x"),
                },
                Err(err) => (&format!("{err}"), "!"),
            };
            let show = match self.errors {
                Errors::Never => false,
                Errors::Last => retry == self.retries,
                Errors::Always => true,
                Errors::Each10 => (retry + 1) % 10 == 0,
            };
            if show {
                if spaces != 4 {
                    println!();
                    spaces = 4;
                }
                println!("    - {full}");
            } else {
                if spaces == 4 {
                    print!("    ");
                }
                print!("{brief}");
                stdout().flush()?;
                spaces = 1;
            }
            retry += 1;
            if self.retries >= 0 && retry > self.retries {
                break Failed;
            }
            std::thread::sleep(Duration::from_millis(wait));
            wait = ((wait as f64 * self.backoff) as u64).min(self.max_wait);
        };
        utils::aborted()?; // avoid printing a verdict in the wrong place if aborted.
        println!("{}{verdict:?}", " ".repeat(spaces));
        Ok(verdict)
    }
}

impl TryFrom<Entry> for Media {
    type Error = anyhow::Error;

    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let (name, _, _) = entry.collection_parts();
        Ok(Media {
            name: name.to_lowercase(),
            verdict: Pending,
        })
    }
}
