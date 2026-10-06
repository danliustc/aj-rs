# autojump (Rust port) integration for fish.
# Load it from ~/.config/fish/config.fish with:  autojump --init fish | source

set -gx AUTOJUMP_SOURCED 1

# Record the directory each time it changes.
function __autojump_add --on-variable PWD
    status --is-command-substitution; and return
    command autojump --add "$PWD" >/dev/null 2>&1 &
    disown 2>/dev/null
end

for cmd in j jc jo jco
    complete -c $cmd -x -a '(command autojump --complete (commandline -t))'
end

# Print the resolved directory for $argv, or complain and fail.
function __autojump_resolve
    set -l output (command autojump $argv)
    if test "$output" != "." -a -d "$output"
        echo $output
        return 0
    end
    echo "autojump: directory '$argv' not found" >&2
    echo "Try `autojump --help` for more information." >&2
    return 1
end

function j
    switch "$argv[1]"
        case '--'
        case '-*'
            command autojump $argv
            return
    end
    set -l output (__autojump_resolve $argv); or return 1
    if isatty stdout
        set_color red
        echo $output
        set_color normal
    else
        echo $output
    end
    cd $output
end

function jc
    switch "$argv[1]"
        case '--'
        case '-*'
            command autojump $argv
            return
    end
    j $PWD $argv
end

function jo
    switch "$argv[1]"
        case '--'
        case '-*'
            command autojump $argv
            return
    end
    set -l output (__autojump_resolve $argv); or return 1
    switch (uname)
        case Darwin
            open $output
        case 'CYGWIN*' 'MSYS*'
            cygstart "" (cygpath -w -a $output)
        case '*'
            xdg-open $output
    end
end

function jco
    switch "$argv[1]"
        case '--'
        case '-*'
            command autojump $argv
            return
    end
    jo $PWD $argv
end
