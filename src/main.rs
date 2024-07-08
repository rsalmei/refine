mod dupes;
mod list;
mod rebuild;
mod utils;

use clap::builder::NonEmptyStringValueParser;
use clap::{Parser, Subcommand};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{atomic, Arc, OnceLock};
use std::{fmt, iter};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, after_help = "For more information, see https://github.com/rsalmei/refine")]
struct Args {
    #[command(subcommand)]
    cmd: Command,
    /// Paths to scan.
    #[arg(global = true, help_heading = Some("Global"))]
    paths: Vec<PathBuf>,
    /// Include these files; tested against filename+extension, case-insensitive.
    #[arg(short, long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    include: Option<String>,
    /// Exclude these files; tested against filename+extension, case-insensitive.
    #[arg(short = 'x', long, global = true, help_heading = Some("Global"), value_name = "REGEX", allow_hyphen_values = true, value_parser = NonEmptyStringValueParser::new())]
    exclude: Option<String>,
    /// Do not recurse into subdirectories.
    #[arg(long, global = true, help_heading = Some("Global"))]
    shallow: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find possibly duplicated files by both size and filename.
    Dupes(dupes::Dupes),
    /// Rebuild the filenames of collections of files intelligently.
    Rebuild(rebuild::Rebuild),
    /// List files from the given paths.
    List(list::List),
}

static RE_IN: OnceLock<Regex> = OnceLock::new();
static RE_EX: OnceLock<Regex> = OnceLock::new();
fn set_re(value: &Option<String>, var: &'static OnceLock<Regex>, param: &str) {
    if let Some(s) = value {
        match Regex::new(&format!("(?i){s}")) {
            Ok(re) => var.set(re).unwrap(),
            Err(err) => {
                eprintln!("error: invalid --{param}: {err}");
                std::process::exit(1);
            }
        }
    }
}

static ARGS: OnceLock<Args> = OnceLock::new();
fn args() -> &'static Args {
    ARGS.get().unwrap()
}

fn main() {
    ARGS.set(Args::parse()).unwrap();
    println!("Refine: v{}", env!("CARGO_PKG_VERSION"));
    set_re(&args().include, &RE_IN, "include");
    set_re(&args().exclude, &RE_EX, "exclude");
    let s = if args().shallow { "not " } else { "" };
    println!("  - paths ({s}recursive): {:?}", args().paths);
    match (args().include.as_ref(), args().exclude.as_ref()) {
        (Some(si), None) => println!("  - include: {si:?}"),
        (None, Some(se)) => println!("  - exclude: {se:?}"),
        (Some(si), Some(se)) => println!("  - include: {si:?}, exclude: {se:?}"),
        (None, None) => {}
    }

    ctrlc::set_handler({
        let running = Arc::clone(utils::running_flag());
        move || {
            eprintln!("aborting...");
            running.store(false, atomic::Ordering::Relaxed);
        }
    })
    .expect("Error setting Ctrl-C handler");

    // lists files from the given paths, or the current directory if no paths were given.
    let cd = args().paths.is_empty().then(|| ".".into());
    let files = args().paths.iter().cloned().chain(cd).flat_map(entries);

    if let Err(err) = match args().cmd {
        Command::Dupes(_) => dupes::run(gen_medias(files)),
        Command::Rebuild(_) => rebuild::run(gen_medias(files)),
        Command::List(_) => list::run(gen_medias(files)),
    } {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}

fn entries(dir: PathBuf) -> Box<dyn Iterator<Item = PathBuf>> {
    fn is_included(path: &Path) -> Option<bool> {
        let name = path.file_name()?.to_str()?;
        (!name.starts_with('.')).then_some(())?; // exclude hidden files and folders.
        match (path.is_dir(), &args().include, &args().exclude) {
            (true, _, _) => Some(!args().shallow),
            (false, None, None) => Some(true),
            (false, Some(_), None) => Some(RE_IN.get().unwrap().is_match(name)),
            (false, None, Some(_)) => Some(!RE_EX.get().unwrap().is_match(name)),
            (false, Some(_), Some(_)) => {
                Some(!RE_EX.get().unwrap().is_match(name) && RE_IN.get().unwrap().is_match(name))
            }
        }
    }

    // now this allows hidden directories, if the user directly asks for them.
    match std::fs::read_dir(&dir) {
        Ok(rd) => Box::new(
            rd.inspect(move |r| {
                if let Err(err) = r {
                    eprintln!("error: read entry {}: {err:?}", dir.display());
                }
            })
            .flatten()
            .flat_map(move |de| {
                let path = de.path();
                match (is_included(&path).unwrap_or_default(), path.is_dir()) {
                    (true, false) => Box::new(iter::once(path)),
                    (true, true) if utils::running() => entries(path),
                    _ => Box::new(iter::empty()),
                }
            }),
        ),
        Err(err) => {
            eprintln!("error: read dir {}: {err:?}", dir.display());
            Box::new(iter::empty())
        }
    }
}

fn gen_medias<T>(files: impl Iterator<Item = PathBuf>) -> Vec<T>
where
    T: TryFrom<PathBuf, Error: fmt::Debug>,
{
    files
        .map(|p| T::try_from(p))
        .inspect(|m| {
            if let Err(err) = m {
                eprintln!("error: load media: {err:?}");
            }
        })
        .flatten()
        .collect()
}
