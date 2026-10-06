//! Directory matching, ported from autojump's `find_matches`.
//!
//! Candidates are ranked by weight, then three strategies are tried in order
//! and their results concatenated (deduplicated):
//!
//! 1. **consecutive** – needles match consecutive path components, the last
//!    needle inside the last component: `foo bar` matches `/x/foo/bar` and
//!    `/x/myfoo/rebar`, but not `/x/foo/y/bar` or `/x/foo/bar/baz`.
//! 2. **fuzzy** – the last needle is similar (difflib ratio ≥ 0.6) to the
//!    last path component.
//! 3. **anywhere** – needles appear anywhere in the path, in order.
//!
//! Matching is smartcase: case-insensitive unless a needle has uppercase.

use std::collections::HashSet;
use std::path::{Path, MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

use regex::{Regex, RegexBuilder};

use crate::data::Entry;
use crate::sequence_matcher;

const FUZZY_THRESHOLD: f64 = 0.6;

/// Case-insensitive unless some needle contains an uppercase letter.
pub fn ignore_case(needles: &[String]) -> bool {
    !needles.iter().any(|n| n.chars().any(char::is_uppercase))
}

fn build_regex(pattern: &str, ignore_case: bool) -> Regex {
    RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .expect("needles are escaped, so the pattern is always valid")
}

fn consecutive_regex(needles: &[String], ignore_case: bool) -> Regex {
    let sep = regex::escape(MAIN_SEPARATOR_STR);
    let no_sep = format!("[^{sep}]*");
    let one_sep = format!("{no_sep}{sep}{no_sep}");
    let body = needles
        .iter()
        .map(|n| regex::escape(n))
        .collect::<Vec<_>>()
        .join(&one_sep);
    build_regex(&format!("{body}{no_sep}$"), ignore_case)
}

fn anywhere_regex(needles: &[String], ignore_case: bool) -> Regex {
    let body = needles
        .iter()
        .map(|n| regex::escape(n))
        .collect::<Vec<_>>()
        .join(".*");
    build_regex(&format!(".*{body}.*"), ignore_case)
}

fn last_component(path: &str) -> &str {
    path.rsplit(MAIN_SEPARATOR).next().unwrap_or(path)
}

fn fuzzy_matches(needle: &str, path: &str, ignore_case: bool) -> bool {
    let ratio = if ignore_case {
        sequence_matcher::ratio(needle, &last_component(path).to_lowercase())
    } else {
        sequence_matcher::ratio(needle, last_component(path))
    };
    ratio >= FUZZY_THRESHOLD
}

/// Lazily yields matching entries, best first.
///
/// `entries` must already be sorted by weight (descending). The current
/// directory is always excluded; with `check_entries`, so are paths that are
/// no longer directories (checked lazily, so a plain jump stats only as many
/// paths as needed).
pub fn find_matches<'a>(
    entries: &'a [Entry],
    needles: &[String],
    check_entries: bool,
) -> impl Iterator<Item = &'a Entry> + 'a {
    let ignore_case = ignore_case(needles);
    let consecutive = consecutive_regex(needles, ignore_case);
    let anywhere = anywhere_regex(needles, ignore_case);
    let fuzzy_needle = {
        let last = needles.last().map(String::as_str).unwrap_or("");
        if ignore_case {
            last.to_lowercase()
        } else {
            last.to_owned()
        }
    };

    let by_consecutive = entries
        .iter()
        .filter(move |e| consecutive.is_match(&e.path));
    let by_fuzzy = entries
        .iter()
        .filter(move |e| fuzzy_matches(&fuzzy_needle, &e.path, ignore_case));
    let by_anywhere = entries.iter().filter(move |e| anywhere.is_match(&e.path));

    let cwd = std::env::current_dir().ok();
    let mut seen = HashSet::new();
    by_consecutive
        .chain(by_fuzzy)
        .chain(by_anywhere)
        .filter(move |e| seen.insert(e.path.as_str()))
        .filter(move |e| !is_cwd(&e.path, cwd.as_deref()))
        .filter(move |e| !check_entries || Path::new(&e.path).is_dir())
}

fn is_cwd(path: &str, cwd: Option<&Path>) -> bool {
    cwd.is_some_and(|cwd| Path::new(path).canonicalize().is_ok_and(|p| p == cwd))
}

/// Parsed form of a tab-completion token such as `foo__2__/home/u/foo`.
#[derive(Debug, Default, PartialEq)]
pub struct TabEntry {
    pub needle: Option<String>,
    pub index: Option<usize>,
    pub path: Option<String>,
}

/// Mirror autojump's `get_tab_entry_info`: `needle__N__path`, where each
/// part is located independently with the same regexes upstream uses.
pub fn parse_tab_entry(token: &str, separator: &str) -> TabEntry {
    let sep = regex::escape(separator);
    let needle = Regex::new(&format!("^(.*?){sep}")).unwrap();
    let index = Regex::new(&format!("{sep}([0-9])")).unwrap();
    let path = Regex::new(&format!("{sep}[0-9]{sep}(.*)")).unwrap();
    let group = |re: &Regex| re.captures(token).map(|c| c[1].to_owned());
    TabEntry {
        needle: group(&needle),
        index: group(&index).and_then(|i| i.parse().ok()),
        path: group(&path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(paths: &[(&str, f64)]) -> Vec<Entry> {
        let mut es: Vec<Entry> = paths
            .iter()
            .map(|&(p, w)| Entry {
                path: p.to_owned(),
                weight: w,
            })
            .collect();
        es.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        es
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn matched(es: &[Entry], needles: &[&str]) -> Vec<String> {
        find_matches(es, &s(needles), false)
            .map(|e| e.path.clone())
            .collect()
    }

    #[test]
    fn smartcase() {
        assert!(ignore_case(&s(&["foo", "bar"])));
        assert!(!ignore_case(&s(&["foo", "Bar"])));
    }

    #[test]
    fn consecutive_requires_last_needle_in_last_component() {
        let es = entries(&[("/x/foo/bar", 10.0), ("/x/foo/bar/baz", 20.0)]);
        // consecutive hit first, then the anywhere hit
        assert_eq!(
            matched(&es, &["foo", "bar"]),
            ["/x/foo/bar", "/x/foo/bar/baz"]
        );
        assert_eq!(
            matched(&es, &["oo", "ba"]),
            ["/x/foo/bar", "/x/foo/bar/baz"]
        );
        // `baz` ~ `bar` is a fuzzy hit (ratio 0.67), ranked after the exact one.
        assert_eq!(matched(&es, &["baz"]), ["/x/foo/bar/baz", "/x/foo/bar"]);
    }

    #[test]
    fn consecutive_is_ordered_by_weight() {
        let es = entries(&[("/b/proj", 30.0), ("/a/proj", 10.0)]);
        assert_eq!(matched(&es, &["proj"]), ["/b/proj", "/a/proj"]);
    }

    #[test]
    fn needle_regex_chars_are_literal() {
        let es = entries(&[("/x/a.b", 1.0), ("/x/aqqqqb", 2.0)]);
        assert_eq!(matched(&es, &["a.b"]), ["/x/a.b"]);
    }

    #[test]
    fn fuzzy_catches_typos() {
        let es = entries(&[("/home/u/documents", 1.0)]);
        assert_eq!(matched(&es, &["dcumnts"]), ["/home/u/documents"]);
        assert!(matched(&es, &["zzz"]).is_empty());
    }

    #[test]
    fn case_sensitive_with_uppercase_needle() {
        let es = entries(&[("/x/Music", 1.0), ("/x/music", 2.0)]);
        assert_eq!(matched(&es, &["Mus"])[0], "/x/Music");
        assert_eq!(matched(&es, &["mus"]), ["/x/music", "/x/Music"]);
    }

    #[test]
    fn empty_needle_matches_everything() {
        let es = entries(&[("/b", 2.0), ("/a", 1.0)]);
        assert_eq!(matched(&es, &[""]), ["/b", "/a"]);
    }

    #[test]
    fn tab_entry_parsing() {
        assert_eq!(parse_tab_entry("foo", "__"), TabEntry::default());
        assert_eq!(
            parse_tab_entry("foo__", "__"),
            TabEntry {
                needle: Some("foo".into()),
                ..Default::default()
            }
        );
        assert_eq!(
            parse_tab_entry("foo__3", "__"),
            TabEntry {
                needle: Some("foo".into()),
                index: Some(3),
                path: None
            }
        );
        assert_eq!(
            parse_tab_entry("foo__3__/a/b__c", "__"),
            TabEntry {
                needle: Some("foo".into()),
                index: Some(3),
                path: Some("/a/b__c".into())
            }
        );
        assert_eq!(
            parse_tab_entry("node___1", "__"),
            TabEntry {
                needle: Some("node".into()),
                index: Some(1),
                path: None
            }
        );
    }
}
