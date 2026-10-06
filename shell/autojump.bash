# autojump (Rust port) integration for bash.
# Load it from ~/.bashrc with:  eval "$(autojump --init bash)"

export AUTOJUMP_SOURCED=1

# Record the current directory on every prompt; time spent in a directory is
# what raises its weight. Runs in the background so the prompt never waits,
# and preserves $? for whatever runs after it in PROMPT_COMMAND.
__autojump_add_to_database() {
    local ret=$?
    (command autojump --add "$PWD" >/dev/null 2>&1 &)
    return $ret
}

if [[ ";${PROMPT_COMMAND[*]:-};" != *";__autojump_add_to_database;"* ]]; then
    __autojump_pc=${PROMPT_COMMAND:-}
    while [[ $__autojump_pc == *[\;[:space:]] ]]; do __autojump_pc=${__autojump_pc%?}; done
    PROMPT_COMMAND="${__autojump_pc:+$__autojump_pc;}__autojump_add_to_database"
    unset __autojump_pc
fi

__autojump_complete() {
    local cur=${COMP_WORDS[COMP_CWORD]}
    local IFS=$'\n'
    COMPREPLY=($(command autojump --complete "$cur"))
}
complete -o default -F __autojump_complete j jc jo jco

# Print the resolved directory for "$@", or complain and fail.
__autojump_resolve() {
    local output
    output=$(command autojump "$@")
    if [[ $output != "." && -d $output ]]; then
        printf '%s\n' "$output"
        return 0
    fi
    printf "autojump: directory '%s' not found\n" "$*" >&2
    printf "Try \`autojump --help\` for more information.\n" >&2
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
        printf '\033[31m%s\033[0m\n' "$output"
    else
        printf '%s\n' "$output"
    fi
    cd -- "$output"
}

# Jump to a child of the current directory.
jc() {
    if [[ $1 == -* && $1 != "--" ]]; then
        command autojump "$@"
        return
    fi
    j "$PWD" "$@"
}

# Open the matched directory in the file manager.
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
        *) printf 'Unknown operating system: %s\n' "$OSTYPE" >&2; return 1 ;;
    esac
}

# Open a child of the current directory in the file manager.
jco() {
    if [[ $1 == -* && $1 != "--" ]]; then
        command autojump "$@"
        return
    fi
    jo "$PWD" "$@"
}
