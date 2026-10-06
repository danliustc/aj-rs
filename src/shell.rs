//! Shell integration scripts, printed by `autojump --init <shell>`.

pub const SUPPORTED: &[&str] = &["bash", "zsh", "fish"];

pub fn script(shell: &str) -> Option<&'static str> {
    match shell {
        "bash" => Some(include_str!("../shell/autojump.bash")),
        "zsh" => Some(include_str!("../shell/autojump.zsh")),
        "fish" => Some(include_str!("../shell/autojump.fish")),
        _ => None,
    }
}
