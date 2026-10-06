//! Hand-rolled argument parsing (no clap: this binary runs on every prompt,
//! so startup time and size matter). Mirrors autojump's argparse interface.

pub const DEFAULT_INCREASE: f64 = 10.0;
pub const DEFAULT_DECREASE: f64 = 15.0;

#[derive(Debug, PartialEq)]
pub enum Action {
    Jump(Vec<String>),
    Add(String),
    Increase(f64),
    Decrease(f64),
    Complete(Vec<String>),
    Purge,
    Stat,
    Init(String),
    Help,
    Version,
}

pub const USAGE: &str = "\
usage: autojump [-h] [-a DIRECTORY] [-i [WEIGHT]] [-d [WEIGHT]] [--complete]
                [--purge] [-s] [-v] [--init SHELL] [DIRECTORY ...]

Automatically jump to directory passed as an argument.

positional arguments:
  DIRECTORY             directory to jump to

options:
  -h, --help            show this help message and exit
  -a, --add DIRECTORY   add path
  -i, --increase [WEIGHT]
                        increase current directory weight (default 10)
  -d, --decrease [WEIGHT]
                        decrease current directory weight (default 15)
  --complete            used for tab completion
  --purge               remove non-existent paths from database
  -s, --stat            show database entries and their key weights
  -v, --version         show version information
  --init SHELL          print shell integration (bash, zsh, fish), e.g.
                        eval \"$(autojump --init bash)\"

Please see autojump(1) man pages for full documentation.";

pub fn parse<I>(args: I) -> Result<Action, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter().peekable();
    let mut positional = Vec::new();
    let mut action: Option<Action> = None;
    let mut complete = false;

    let set = |slot: &mut Option<Action>, new: Action| -> Result<(), String> {
        if slot.is_some() {
            return Err("only one action option may be given".into());
        }
        *slot = Some(new);
        Ok(())
    };

    while let Some(arg) = args.next() {
        if arg == "--" {
            positional.extend(args.by_ref());
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            positional.push(arg);
            continue;
        }

        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        // Short options may carry their value attached: `-i20`, `-a/tmp`.
        let (flag, inline) = match (flag.as_str(), inline) {
            (f, None) if f.len() > 2 && !f.starts_with("--") => {
                let (f, v) = f.split_at(2);
                (f.to_owned(), Some(v.to_owned()))
            }
            (_, v) => (flag, v),
        };

        match flag.as_str() {
            "-h" | "--help" => set(&mut action, Action::Help)?,
            "-v" | "--version" => set(&mut action, Action::Version)?,
            "-s" | "--stat" => set(&mut action, Action::Stat)?,
            "--purge" => set(&mut action, Action::Purge)?,
            "--complete" => complete = true,
            "-a" | "--add" => {
                let dir = inline
                    .or_else(|| args.next())
                    .ok_or("argument -a/--add: expected one argument")?;
                set(&mut action, Action::Add(dir))?;
            }
            "--init" => {
                let shell = inline
                    .or_else(|| args.next())
                    .ok_or("argument --init: expected one argument")?;
                set(&mut action, Action::Init(shell))?;
            }
            "-i" | "--increase" | "-d" | "--decrease" => {
                let increase = flag == "-i" || flag == "--increase";
                let weight = match inline {
                    Some(v) => parse_weight(&v)?,
                    // argparse `nargs='?'`: take the next arg only if it is a value.
                    None => match args.peek().and_then(|v| v.parse::<f64>().ok()) {
                        Some(w) => {
                            args.next();
                            check_weight(w)?
                        }
                        None if increase => DEFAULT_INCREASE,
                        None => DEFAULT_DECREASE,
                    },
                };
                let act = if increase {
                    Action::Increase(weight)
                } else {
                    Action::Decrease(weight)
                };
                set(&mut action, act)?;
            }
            _ => return Err(format!("unrecognized arguments: {arg}")),
        }
    }

    match action {
        Some(_) if complete => Err("--complete cannot be combined with other options".into()),
        Some(a) => Ok(a),
        None if complete => Ok(Action::Complete(positional)),
        None => Ok(Action::Jump(positional)),
    }
}

fn parse_weight(v: &str) -> Result<f64, String> {
    v.parse::<f64>()
        .map_err(|_| format!("invalid weight value: '{v}'"))
        .and_then(check_weight)
}

fn check_weight(w: f64) -> Result<f64, String> {
    if w.is_finite() && w >= 0.0 {
        Ok(w)
    } else {
        Err(format!("invalid weight value: '{w}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Action, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn jump_and_complete() {
        assert_eq!(p(&[]), Ok(Action::Jump(vec![])));
        assert_eq!(
            p(&["foo", "bar"]),
            Ok(Action::Jump(vec!["foo".into(), "bar".into()]))
        );
        assert_eq!(
            p(&["--complete", "fo"]),
            Ok(Action::Complete(vec!["fo".into()]))
        );
        assert_eq!(
            p(&["--", "-weird"]),
            Ok(Action::Jump(vec!["-weird".into()]))
        );
    }

    #[test]
    fn optional_weights() {
        assert_eq!(p(&["-i"]), Ok(Action::Increase(10.0)));
        assert_eq!(p(&["-i", "20"]), Ok(Action::Increase(20.0)));
        assert_eq!(p(&["-i20"]), Ok(Action::Increase(20.0)));
        assert_eq!(p(&["--decrease=3"]), Ok(Action::Decrease(3.0)));
        assert_eq!(p(&["-d"]), Ok(Action::Decrease(15.0)));
        assert!(p(&["-i", "-5"]).is_err());
    }

    #[test]
    fn add_and_errors() {
        assert_eq!(p(&["-a", "/tmp"]), Ok(Action::Add("/tmp".into())));
        assert_eq!(p(&["--add=/tmp"]), Ok(Action::Add("/tmp".into())));
        assert!(p(&["-a"]).is_err());
        assert!(p(&["--bogus"]).is_err());
        assert!(p(&["-s", "--purge"]).is_err());
        assert_eq!(p(&["--init", "zsh"]), Ok(Action::Init("zsh".into())));
    }
}
