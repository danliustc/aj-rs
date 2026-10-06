#!/usr/bin/env bash
# End-to-end smoke test of the shell integration in every shell available.
#
# Usage: tools/shell-smoke.sh DIR_CONTAINING_AUTOJUMP_BINARY
#
# For each of bash, zsh and fish: load `autojump --init <shell>`, visit some
# directories, then check j / jc / not-found handling / tab completion.
set -u

BIN_DIR=$(cd "$1" && pwd)
T=$(mktemp -d)
T=$(cd "$T" && pwd) # logical path, as the shells will see it
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/w/projects/aj-rs" "$T/w/docs" "$T/home"

# Shells add directories in the background; wait until both have landed.
cat >"$T/wait.sh" <<WAIT
#!/bin/sh
for _ in \$(seq 1 50); do
    if grep -q "aj-rs\$" "$T/data/autojump.txt" 2>/dev/null &&
        grep -q "docs\$" "$T/data/autojump.txt" 2>/dev/null; then
        exit 0
    fi
    sleep 0.1
done
echo "timed out waiting for database writes" >&2
exit 1
WAIT
chmod +x "$T/wait.sh"

expected="$T/w/projects/aj-rs
$T/w/docs
not-found
aj__1__$T/w/projects/aj-rs
hook-ok"

export HOME="$T/home" AUTOJUMP_DATA_DIR="$T/data" PATH="$BIN_DIR:$PATH"

run_bash() {
    bash --norc --noprofile <<SCRIPT
eval "\$(autojump --init bash)"
eval "\$(autojump --init bash)"
cd "$T/w/projects/aj-rs"; __autojump_add_to_database
cd "$T/w/docs"; __autojump_add_to_database
"$T/wait.sh" || exit 1
cd /; j aj >/dev/null && pwd
cd "$T/w" && jc doc >/dev/null && pwd
j zzzz 2>/dev/null || echo not-found
COMP_WORDS=(j aj); COMP_CWORD=1; __autojump_complete; printf '%s\n' "\${COMPREPLY[@]}"
false; __autojump_add_to_database; st=\$?
[[ \$PROMPT_COMMAND == __autojump_add_to_database && \$st == 1 ]] && echo hook-ok
SCRIPT
}

run_zsh() {
    zsh -f <<SCRIPT
autoload -Uz compinit && compinit -u -D
eval "\$(autojump --init zsh)"
cd "$T/w/projects/aj-rs"; cd "$T/w/docs"
"$T/wait.sh" || exit 1
cd /; j aj >/dev/null && pwd
cd "$T/w" && jc doc >/dev/null && pwd
j zzzz 2>/dev/null || echo not-found
command autojump --complete aj
[[ \${_comps[j]} == __autojump_complete && \${chpwd_functions[(I)__autojump_chpwd]} -gt 0 ]] && echo hook-ok
SCRIPT
}

run_fish() {
    fish --no-config <<SCRIPT
autojump --init fish | source
cd "$T/w/projects/aj-rs"; cd "$T/w/docs"
"$T/wait.sh"; or exit 1
cd /; j aj >/dev/null; and pwd
cd "$T/w"; and jc doc >/dev/null; and pwd
j zzzz 2>/dev/null; or echo not-found
complete -C 'j aj'
functions -q __autojump_add; and echo hook-ok
SCRIPT
}

status=0
for sh in bash zsh fish; do
    if ! command -v "$sh" >/dev/null; then
        echo "SKIP $sh (not installed)"
        continue
    fi
    rm -rf "$T/data"
    actual=$("run_$sh" 2>&1)
    if [ "$actual" = "$expected" ]; then
        echo "PASS $sh ($("$sh" --version | head -n1))"
    else
        echo "FAIL $sh"
        diff <(echo "$expected") <(echo "$actual")
        status=1
    fi
done
exit $status
