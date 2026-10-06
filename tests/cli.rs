//! End-to-end tests driving the `autojump` binary against a temp data dir.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Env {
    data: TempDir,
    root: TempDir,
}

impl Env {
    fn new() -> Self {
        Env {
            data: TempDir::new().unwrap(),
            root: TempDir::new().unwrap(),
        }
    }

    fn dir(&self, rel: &str) -> String {
        let p = self.root.path().join(rel);
        fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn data_file(&self) -> PathBuf {
        self.data.path().join("autojump.txt")
    }

    fn run_in(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_autojump"))
            .args(args)
            .current_dir(cwd)
            .env("AUTOJUMP_DATA_DIR", self.data.path())
            .env("PWD", cwd)
            .output()
            .unwrap()
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.run_in(self.root.path(), args);
        assert!(out.status.success(), "{args:?} failed: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }
}

#[test]
fn add_then_jump() {
    let env = Env::new();
    let proj = env.dir("code/project");
    let docs = env.dir("documents");
    env.run(&["-a", &proj]);
    env.run(&["-a", &docs]);
    env.run(&["-a", &docs]);

    assert_eq!(env.run(&["proj"]).trim(), proj);
    assert_eq!(env.run(&["code", "proj"]).trim(), proj);
    assert_eq!(env.run(&["dcuments"]).trim(), docs, "fuzzy match");
    // no needles: the heaviest entry
    assert_eq!(env.run(&[]).trim(), docs);
    assert_eq!(env.run(&["nothing-like-this"]).trim(), ".");
}

#[test]
fn data_file_is_autojump_compatible() {
    let env = Env::new();
    let a = env.dir("a");
    env.run(&["--add", &a]);
    env.run(&["--add", &format!("{a}/")]);
    let text = fs::read_to_string(env.data_file()).unwrap();
    assert_eq!(text, format!("14.142135623730951\t{a}\n"));
    assert!(env.data.path().join("autojump.txt.bak").exists());
}

#[test]
fn reads_existing_python_database() {
    let env = Env::new();
    let low = env.dir("low");
    let high = env.dir("high");
    fs::write(
        env.data_file(),
        format!("5.0\t{low}\n22.360679774997898\t{high}\ngarbage line\n"),
    )
    .unwrap();
    assert_eq!(env.run(&[]).trim(), high);
}

#[test]
fn skips_missing_dirs_and_cwd() {
    let env = Env::new();
    let gone = env.root.path().join("gone/foo");
    let foo1 = env.dir("one/foo");
    let foo2 = env.dir("two/foo");
    fs::write(
        env.data_file(),
        format!("100\t{}\n50\t{foo1}\n10\t{foo2}\n", gone.to_string_lossy()),
    )
    .unwrap();
    assert_eq!(env.run(&["foo"]).trim(), foo1);
    let out = env.run_in(Path::new(&foo1), &["foo"]);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), foo2);
}

#[test]
fn increase_decrease_and_stat() {
    let env = Env::new();
    let here = env.dir("here");
    let cwd = Path::new(&here);
    let out = env.run_in(cwd, &["-i", "20"]);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("20.0:\t{here}\n")
    );
    let out = env.run_in(cwd, &["-d"]);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("5.0:\t{here}\n")
    );

    let stat = String::from_utf8(env.run_in(cwd, &["-s"]).stdout).unwrap();
    assert!(stat.contains(&format!("5.0:\t{here}\n")), "{stat}");
    assert!(stat.contains("5:\t total weight"), "{stat}");
    assert!(stat.contains("1:\t number of entries"), "{stat}");
    assert!(stat.contains("5.00:\t current directory weight"), "{stat}");
}

#[test]
fn purge_removes_missing() {
    let env = Env::new();
    let keep = env.dir("keep");
    fs::write(env.data_file(), format!("10\t{keep}\n10\t/no/such/dir/x\n")).unwrap();
    assert_eq!(env.run(&["--purge"]), "Purged 1 entries.\n");
    let text = fs::read_to_string(env.data_file()).unwrap();
    assert_eq!(text, format!("10.0\t{keep}\n"));
}

#[test]
fn tab_completion_roundtrip() {
    let env = Env::new();
    let a = env.dir("x/foo");
    let b = env.dir("y/foobar");
    fs::write(env.data_file(), format!("30\t{a}\n20\t{b}\n")).unwrap();

    let menu = env.run(&["--complete", "foo"]);
    assert_eq!(menu, format!("foo__1__{a}\nfoo__2__{b}\n"));
    // selecting a menu entry resolves to its path
    assert_eq!(env.run(&[&format!("foo__2__{b}")]).trim(), b);
    assert_eq!(env.run(&["foo__2"]).trim(), b);
    assert_eq!(env.run(&["foo__"]).trim(), a);
    assert_eq!(env.run(&["--complete", "foo__2"]).trim(), b);
}

#[test]
fn explicit_paths_bypass_database() {
    let env = Env::new();
    let sub = env.dir("sub");
    let other = env.dir("elsewhere/sub");
    fs::write(env.data_file(), format!("10\t{other}\n")).unwrap();
    assert_eq!(env.run(&["./sub"]).trim(), "./sub");
    assert_eq!(env.run(&[&sub]).trim(), sub);
    // a bare word still consults the database
    assert_eq!(env.run(&["sub"]).trim(), other);
}

#[test]
fn home_is_never_recorded() {
    let env = Env::new();
    let home = env.dir("home");
    let out = Command::new(env!("CARGO_BIN_EXE_autojump"))
        .args(["-a", &home])
        .env("AUTOJUMP_DATA_DIR", env.data.path())
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(fs::read_to_string(env.data_file()).unwrap(), "");
}

#[test]
fn init_scripts() {
    let env = Env::new();
    for shell in ["bash", "zsh", "fish"] {
        assert!(env.run(&["--init", shell]).contains("autojump --add"));
    }
    let out = env.run_in(env.root.path(), &["--init", "tcsh"]);
    assert!(!out.status.success());
}

#[test]
fn bad_arguments_fail() {
    let env = Env::new();
    let out = env.run_in(env.root.path(), &["--bogus"]);
    assert_eq!(out.status.code(), Some(2));
}
