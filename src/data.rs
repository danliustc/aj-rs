//! The weighted directory database.
//!
//! On-disk format is identical to the original autojump (`weight\tpath` per
//! line, UTF-8), so an existing `autojump.txt` can be used as-is.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The backup file is refreshed at most once per this interval.
const BACKUP_THRESHOLD: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub weight: f64,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub data_path: PathBuf,
    pub backup_path: PathBuf,
}

impl Config {
    /// Resolve the data directory the same way autojump does, with an extra
    /// `AUTOJUMP_DATA_DIR` override (handy for tests and custom setups).
    pub fn from_env() -> io::Result<Self> {
        let dir = match std::env::var_os("AUTOJUMP_DATA_DIR").filter(|v| !v.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => default_data_dir()?,
        };
        Ok(Self::in_dir(&dir))
    }

    pub fn in_dir(dir: &Path) -> Self {
        Config {
            data_path: dir.join("autojump.txt"),
            backup_path: dir.join("autojump.txt.bak"),
        }
    }

    fn lock_path(&self) -> PathBuf {
        self.data_path.with_extension("txt.lock")
    }
}

fn home_dir() -> io::Result<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("${var} is not set")))
}

fn default_data_dir() -> io::Result<PathBuf> {
    if cfg!(target_os = "macos") {
        return Ok(home_dir()?.join("Library").join("autojump"));
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(appdata).join("autojump"));
        }
    }
    let base = match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => home_dir()?.join(".local").join("share"),
    };
    Ok(base.join("autojump"))
}

/// path -> weight
#[derive(Debug, Default, Clone)]
pub struct Database {
    pub weights: HashMap<String, f64>,
}

impl Database {
    /// Load the database, falling back to the backup if the main file is
    /// missing. A missing database is simply empty.
    pub fn load(config: &Config) -> io::Result<Self> {
        let content = match fs::read(&config.data_path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => match fs::read(&config.backup_path) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
                Err(e) => return Err(e),
            },
            Err(e) => return Err(e),
        };
        Ok(Self::parse(&String::from_utf8_lossy(&content)))
    }

    pub fn parse(content: &str) -> Self {
        let mut weights = HashMap::new();
        for line in content.lines() {
            let Some((weight, path)) = line.split_once('\t') else {
                continue;
            };
            let Ok(weight) = weight.trim().parse::<f64>() else {
                continue;
            };
            if path.is_empty() || !weight.is_finite() {
                continue;
            }
            weights.insert(path.to_owned(), weight);
        }
        Database { weights }
    }

    /// Entries sorted by weight, heaviest first; ties broken by path in
    /// descending order, exactly like autojump's `sorted(key=(weight, path),
    /// reverse=True)`.
    pub fn entries(&self) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .weights
            .iter()
            .map(|(path, &weight)| Entry {
                path: path.clone(),
                weight,
            })
            .collect();
        entries.sort_by(|a, b| {
            b.weight
                .total_cmp(&a.weight)
                .then_with(|| b.path.cmp(&a.path))
        });
        entries
    }

    /// Increase a path's weight: `sqrt(old^2 + inc^2)`.
    /// The home directory is never recorded (it is always one `cd` away).
    pub fn add(&mut self, path: &str, increment: f64) -> Entry {
        let path = normalize(path);
        if is_home(&path) {
            return Entry { path, weight: 0.0 };
        }
        let old = self.weights.get(&path).copied().unwrap_or(0.0);
        let weight = (old * old + increment * increment).sqrt();
        self.weights.insert(path.clone(), weight);
        Entry { path, weight }
    }

    /// Decrease a path's weight linearly, never below zero.
    pub fn decrease(&mut self, path: &str, decrement: f64) -> Entry {
        let path = normalize(path);
        let old = self.weights.get(&path).copied().unwrap_or(0.0);
        let weight = (old - decrement).max(0.0);
        self.weights.insert(path.clone(), weight);
        Entry { path, weight }
    }

    /// Drop entries whose directory no longer exists. Returns how many.
    pub fn purge(&mut self) -> usize {
        let before = self.weights.len();
        self.weights.retain(|path, _| Path::new(path).exists());
        before - self.weights.len()
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for entry in self.entries() {
            out.push_str(&format_weight(entry.weight));
            out.push('\t');
            out.push_str(&entry.path);
            out.push('\n');
        }
        out
    }

    /// Atomically write the database (temp file + rename) and refresh the
    /// daily backup.
    pub fn save(&self, config: &Config) -> io::Result<()> {
        let dir = config.data_path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(dir)?;

        let tmp = dir.join(format!(".autojump.txt.{}.tmp", std::process::id()));
        {
            let mut file = BufWriter::new(File::create(&tmp)?);
            file.write_all(self.serialize().as_bytes())?;
            file.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        }
        if let Err(e) = fs::rename(&tmp, &config.data_path) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }

        if backup_is_stale(&config.backup_path) {
            fs::copy(&config.data_path, &config.backup_path)?;
        }
        Ok(())
    }
}

fn backup_is_stale(backup: &Path) -> bool {
    let Ok(modified) = fs::metadata(backup).and_then(|m| m.modified()) else {
        return true;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age > BACKUP_THRESHOLD)
        .unwrap_or(false)
}

/// Exclusive advisory lock around a load-modify-save cycle, so concurrent
/// prompt hooks (several shells open) don't lose each other's updates.
pub struct Lock(#[allow(dead_code)] File);

impl Lock {
    pub fn acquire(config: &Config) -> io::Result<Self> {
        if let Some(dir) = config.data_path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(config.lock_path())?;
        file.lock()?;
        Ok(Lock(file))
    }
}

/// Strip trailing separators, keeping a bare root intact.
pub fn normalize(path: &str) -> String {
    let trimmed = path.trim_end_matches(std::path::MAIN_SEPARATOR);
    if trimmed.is_empty() && !path.is_empty() {
        path[..std::path::MAIN_SEPARATOR.len_utf8()].to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn is_home(path: &str) -> bool {
    home_dir()
        .map(|home| normalize(&home.to_string_lossy()) == path)
        .unwrap_or(false)
}

/// Format like Python's `str(float)` for the common cases (`10.0`, `14.142…`).
pub fn format_weight(weight: f64) -> String {
    if weight.fract() == 0.0 && weight.abs() < 1e16 {
        format!("{weight:.1}")
    } else {
        format!("{weight}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skips_garbage() {
        let db = Database::parse("10.0\t/a\nnot a number\t/b\n\n5\t/c\nno-tab\n3.5\t/with\ttab\n");
        assert_eq!(db.weights.len(), 3);
        assert_eq!(db.weights["/a"], 10.0);
        assert_eq!(db.weights["/c"], 5.0);
        assert_eq!(db.weights["/with\ttab"], 3.5);
    }

    #[test]
    fn add_uses_quadratic_growth() {
        let mut db = Database::default();
        assert_eq!(db.add("/tmp/x/", 10.0).weight, 10.0);
        let e = db.add("/tmp/x", 10.0);
        assert!((e.weight - 200f64.sqrt()).abs() < 1e-12);
        assert_eq!(e.path, "/tmp/x");
    }

    #[test]
    fn decrease_floors_at_zero() {
        let mut db = Database::default();
        db.add("/tmp/x", 10.0);
        assert_eq!(db.decrease("/tmp/x", 15.0).weight, 0.0);
    }

    #[test]
    fn serialize_roundtrip_sorted() {
        let mut db = Database::default();
        db.weights.insert("/low".into(), 1.0);
        db.weights.insert("/high".into(), 200f64.sqrt());
        let text = db.serialize();
        assert_eq!(text, "14.142135623730951\t/high\n1.0\t/low\n");
        assert_eq!(Database::parse(&text).weights, db.weights);
    }

    #[test]
    fn normalize_keeps_root() {
        assert_eq!(normalize("/"), "/");
        assert_eq!(normalize("///"), "/");
        assert_eq!(normalize("/a/b//"), "/a/b");
    }
}
