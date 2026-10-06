//! autojump: a faster way to navigate your filesystem — Rust port.

mod cli;
mod data;
mod matching;
mod sequence_matcher;
mod shell;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cli::Action;
use data::{Config, Database, Entry, Lock};

const TAB_SEPARATOR: &str = "__";
const TAB_ENTRIES_COUNT: usize = 9;

fn main() -> ExitCode {
    let action = match cli::parse(
        std::env::args_os()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned()),
    ) {
        Ok(action) => action,
        Err(msg) => {
            eprintln!(
                "{}\nautojump: error: {msg}",
                cli::USAGE.lines().take(2).collect::<Vec<_>>().join("\n")
            );
            return ExitCode::from(2);
        }
    };
    match run(action) {
        Ok(()) => ExitCode::SUCCESS,
        // Writing into a closed pipe (`autojump -s | head`) is not an error.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("autojump: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(action: Action) -> io::Result<()> {
    let mut out = io::stdout().lock();
    match action {
        Action::Help => writeln!(out, "{}", cli::USAGE),
        Action::Version => writeln!(out, "autojump v{} (Rust port)", env!("CARGO_PKG_VERSION")),
        Action::Init(name) => match shell::script(&name) {
            Some(script) => out.write_all(script.as_bytes()),
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unsupported shell '{name}' (expected one of: {})",
                    shell::SUPPORTED.join(", ")
                ),
            )),
        },
        Action::Add(dir) => {
            let config = Config::from_env()?;
            modify(&config, |db| {
                db.add(&dir, cli::DEFAULT_INCREASE);
            })
        }
        Action::Increase(weight) => {
            let config = Config::from_env()?;
            let cwd = logical_cwd()?;
            let entry = modify(&config, |db| db.add(&cwd, weight))?;
            print_entry(&mut out, &entry)
        }
        Action::Decrease(weight) => {
            let config = Config::from_env()?;
            let cwd = logical_cwd()?;
            let entry = modify(&config, |db| db.decrease(&cwd, weight))?;
            print_entry(&mut out, &entry)
        }
        Action::Purge => {
            let config = Config::from_env()?;
            let purged = modify(&config, Database::purge)?;
            writeln!(out, "Purged {purged} entries.")
        }
        Action::Stat => {
            let config = Config::from_env()?;
            print_stats(&mut out, &Database::load(&config)?, &config)
        }
        Action::Complete(needles) => {
            let config = Config::from_env()?;
            let entries = Database::load(&config)?.entries();
            let needle = needles.first().map(|n| sanitize(n)).unwrap_or_default();
            complete(&mut out, &entries, &needle)
        }
        Action::Jump(needles) => {
            let target = resolve_jump(&needles)?;
            writeln!(out, "{target}")
        }
    }
}

/// Load, mutate and save the database while holding the lock.
fn modify<T>(config: &Config, f: impl FnOnce(&mut Database) -> T) -> io::Result<T> {
    let _lock = Lock::acquire(config)?;
    let mut db = Database::load(config)?;
    let result = f(&mut db);
    db.save(config)?;
    Ok(result)
}

fn resolve_jump(needles: &[String]) -> io::Result<String> {
    // An explicit path (`j ..`, `j /tmp`, `j ./src`) wins outright. A bare
    // word like `j foo` still goes through the database even if `./foo`
    // exists.
    if let [arg] = needles {
        if looks_like_path(arg) && Path::new(arg).is_dir() {
            return Ok(arg.clone());
        }
    }

    let config = Config::from_env()?;
    let entries = Database::load(&config)?.entries();
    let needles: Vec<String> = if needles.is_empty() {
        vec![String::new()]
    } else {
        needles.iter().map(|n| sanitize(n)).collect()
    };

    // Support jumping straight from a tab-completion token (`foo__2`), with
    // a bare `foo__` meaning the first entry.
    let mut tab = matching::parse_tab_entry(&needles[0], TAB_SEPARATOR);
    if tab.path.is_none() && tab.index.is_none() {
        if let Some(needle) = &tab.needle {
            if needles[0] == format!("{needle}{TAB_SEPARATOR}") {
                tab.index = Some(1);
            }
        }
    }
    let found = if let Some(path) = tab.path {
        Some(path)
    } else if let (Some(needle), Some(index)) = (tab.needle, tab.index) {
        nth_match(&entries, &needle, index, true)
    } else {
        matching::find_matches(&entries, &needles, true)
            .next()
            .map(|e| e.path.clone())
    };
    Ok(found.unwrap_or_else(|| ".".to_owned()))
}

fn complete(out: &mut impl Write, entries: &[Entry], needle: &str) -> io::Result<()> {
    let tab = matching::parse_tab_entry(needle, TAB_SEPARATOR);
    if let Some(path) = tab.path {
        return writeln!(out, "{path}");
    }
    if let (Some(needle), Some(index)) = (&tab.needle, tab.index) {
        if let Some(path) = nth_match(entries, needle, index, false) {
            writeln!(out, "{path}")?;
        }
        return Ok(());
    }
    let needle = tab.needle.as_deref().unwrap_or(needle);
    let needles = [needle.to_owned()];
    for (i, entry) in matching::find_matches(entries, &needles, false)
        .take(TAB_ENTRIES_COUNT)
        .enumerate()
    {
        writeln!(
            out,
            "{needle}{TAB_SEPARATOR}{}{TAB_SEPARATOR}{}",
            i + 1,
            entry.path
        )?;
    }
    Ok(())
}

/// The `index`-th (1-based) match, or the last one if there are fewer.
fn nth_match(entries: &[Entry], needle: &str, index: usize, check: bool) -> Option<String> {
    let needles = [needle.to_owned()];
    matching::find_matches(entries, &needles, check)
        .take(index.max(1))
        .last()
        .map(|e| e.path.clone())
}

fn looks_like_path(arg: &str) -> bool {
    let sep = std::path::MAIN_SEPARATOR;
    arg == "."
        || arg == ".."
        || arg.starts_with(sep)
        || arg.starts_with(&format!(".{sep}"))
        || arg.starts_with(&format!("..{sep}"))
}

fn sanitize(needle: &str) -> String {
    data::normalize(needle)
}

/// `$PWD` if it points at the current directory (keeps symlinked paths the
/// way the user sees them, matching what the shell hooks record), else the
/// physical cwd.
fn logical_cwd() -> io::Result<String> {
    let physical = std::env::current_dir()?;
    if let Some(pwd) = std::env::var_os("PWD").map(PathBuf::from) {
        if pwd.is_absolute() && same_file(&pwd, &physical) {
            return Ok(pwd.to_string_lossy().into_owned());
        }
    }
    Ok(physical.to_string_lossy().into_owned())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn print_entry(out: &mut impl Write, entry: &Entry) -> io::Result<()> {
    writeln!(out, "{:.1}:\t{}", entry.weight, entry.path)
}

fn print_stats(out: &mut impl Write, db: &Database, config: &Config) -> io::Result<()> {
    let mut entries = db.entries();
    entries.reverse();
    for entry in &entries {
        print_entry(out, entry)?;
    }
    let total: f64 = db.weights.values().sum();
    writeln!(out, "________________________________________\n")?;
    writeln!(out, "{}:\t total weight", total as i64)?;
    writeln!(out, "{}:\t number of entries", db.weights.len())?;
    if let Ok(cwd) = logical_cwd() {
        let weight = db.weights.get(&cwd).copied().unwrap_or(0.0);
        writeln!(out, "{weight:.2}:\t current directory weight")?;
    }
    writeln!(out, "\ndata:\t {}", config.data_path.display())
}
