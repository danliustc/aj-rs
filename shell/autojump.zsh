# autojump (Rust port) integration for zsh.
# Load it from ~/.zshrc with:  eval "$(autojump --init zsh)"
# (after compinit, so completion gets registered).

export AUTOJUMP_SOURCED=1

# Record the directory each time it changes.
__autojump_chpwd() {
    command autojump --add "$PWD" >/dev/null 2>&1 &!
}
autoload -Uz add-zsh-hook
add-zsh-hook chpwd __autojump_chpwd

__autojump_complete() {
    local -a comps
    comps=(${(f)"$(command autojump --complete "${words[CURRENT]}")"})
    compadd -U -V autojump -- $comps
}
if (( $+functions[compdef] )); then
    compdef __autojump_complete j jc jo jco
fi

# Print the resolved directory for "$@", or complain and fail.
__autojump_resolve() {
    local output
    output=$(command autojump "$@")
    if [[ $output != "." && -d $output ]]; then
        print -r -- "$output"
        return 0
    fi
    print -r -- "autojump: directory '$*' not found" >&2
    print -r -- "Try \`autojump --help\` for more information." >&2
    return 1
}

j() {
    if [[ $1 == -* && $1 != "--" ]]; then
        command autojump "$@"
        return
    fi
    local output
    output=$(__autojump_resolve "$@") || return 1
    if [[ -t 1 ]]; then
        print -r -- $'\e[31m'"$output"$'\e[0m'
    else
        print -r -- "$output"
    fi
    cd -- "$output"
}

jc() {
    if [[ $1 == -* && $1 != "--" ]]; then
        command autojump "$@"
        return
    fi
    j "$PWD" "$@"
}

jo() {
    if [[ $1 == -* && $1 != "--" ]]; then
        command autojump "$@"
        return
    fi
    local output
    output=$(__autojump_resolve "$@") || return 1
    case $OSTYPE in
        linux* | *bsd*) xdg-open "$output" ;;
        darwin*) open "$output" ;;
        cygwin | msys) cygstart "" "$(cygpath -w -a "$output")" ;;
        *) print -r -- "Unknown operating system: $OSTYPE" >&2; return 1 ;;
    esac
}

jco() {
    if [[ $1 == -* && $1 != "--" ]]; then
        command autojump "$@"
        return
    fi
    jo "$PWD" "$@"
}
