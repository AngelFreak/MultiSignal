#!/usr/bin/env bash
# multisignal.sh: manage Signal Desktop profiles from the terminal.
#
# Uses the same folders and app menu entries as the Signal Profiles app, so
# both can be used side by side: ~/Signal/<name>/ holds each profile, and
# ~/.local/share/applications/Signal-<name>.desktop is its menu entry.
set -euo pipefail
shopt -s nullglob
export LC_ALL=C

SIGNAL_BASE="$HOME/Signal"
APPLICATIONS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
SIGNAL_BIN="${MULTISIGNAL_SIGNAL_BIN:-/snap/bin/signal-desktop}"
PROC="${MULTISIGNAL_PROC:-/proc}"
SNAP_HINT=/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop
SIGNAL_ICON=/snap/signal-desktop/current/meta/gui/signal-desktop.png

usage() {
    cat <<'EOF'
Usage: multisignal.sh COMMAND [ARGS]

Run several Signal Desktop accounts side by side, each with its own messages
and app menu entry.

Commands:
  list                   Show profiles: running or not, size, app menu entry
  create NAME            Create a profile and its app menu entry
  open NAME [LINK]       Start Signal for a profile, optionally with a
                         sgnl:// or signalcaptcha:// link
  delete [--yes] NAME    Move a profile and its app menu entry to the Trash
  repair                 Rewrite this tool's app menu entries, add missing ones
  help                   Show this help

Names use letters, digits, dot, underscore and dash, and start with a letter
or digit.
EOF
}

die() {
    echo "multisignal.sh: $*" >&2
    exit 1
}

valid_name() {
    [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]]
}

trim() {
    local s=$1
    s=${s#"${s%%[![:space:]]*}"}
    echo "${s%"${s##*[![:space:]]}"}"
}

# "My Work!" -> "My-Work", or nothing if no usable name is left.
suggest() {
    local input out="" c i
    input=$(trim "$1")
    for ((i = 0; i < ${#input}; i++)); do
        c=${input:i:1}
        if [[ $c == [A-Za-z0-9._] ]]; then
            out+=$c
        elif [[ $c == [[:space:]-] && $out != *- ]]; then
            out+=-
        fi
    done
    out=${out#"${out%%[A-Za-z0-9]*}"}
    out=${out:0:64}
    while [[ $out == *- ]]; do out=${out%-}; done
    if valid_name "$out"; then echo "$out"; fi
}

profile_dir() { echo "$SIGNAL_BASE/$1"; }
own_launcher() { echo "$APPLICATIONS/Signal-$1.desktop"; }

# Profile names: the valid folder names in ~/Signal, sorted ignoring case.
list_names() {
    local d name
    for d in "$SIGNAL_BASE"/*/; do
        d=${d%/}
        [[ -L $d ]] && continue
        name=${d##*/}
        valid_name "$name" && printf '%s\t%s\n' "${name,,}" "$name"
    done | sort | cut -f2
}

# The existing profile whose name matches ignoring case, if any.
existing_ignoring_case() {
    local want=${1,,} name
    while IFS= read -r name; do
        if [[ ${name,,} == "$want" ]]; then
            echo "$name"
            return
        fi
    done < <(list_names)
}

# The data directories of running Signal processes, one per line. Signal
# (Chromium) rewrites its command line into one space-joined string, so both
# that and the usual NUL-separated form are understood.
running_dirs() {
    local f raw args a joined argv0 value
    for f in "$PROC"/[0-9]*/cmdline; do
        args=()
        mapfile -d '' -t raw 2>/dev/null <"$f" || continue
        for a in "${raw[@]}"; do [[ -n $a ]] && args+=("$a"); done
        [[ ${#args[@]} -gt 0 ]] || continue
        if [[ ${#args[@]} -eq 1 && ${args[0]} == *" --"* ]]; then
            joined=${args[0]}
            argv0=${joined%% --*}
            [[ ${argv0##*/} == signal-desktop && $joined == *" --user-data-dir="* ]] || continue
            value=${joined#*" --user-data-dir="}
            echo "${value%% --*}"
        else
            [[ ${args[0]##*/} == signal-desktop ]] || continue
            for a in "${args[@]:1}"; do
                if [[ $a == --user-data-dir=* ]]; then
                    echo "${a#--user-data-dir=}"
                    break
                fi
            done
        fi
    done
}

is_running() {
    local dir dirs
    dir=$(profile_dir "$1")
    dirs=$(running_dirs)
    grep -qxF -- "$dir" <<<"$dirs"
}

# Every .desktop file (ours, legacy or hand-made) whose Exec line starts
# Signal with this profile's folder.
find_launchers() {
    local needle="--user-data-dir=$(profile_dir "$1")" f line rest after
    for f in "$APPLICATIONS"/*.desktop; do
        while IFS= read -r line; do
            [[ $line == Exec=* ]] || continue
            rest=$line
            while [[ $rest == *"$needle"* ]]; do
                after=${rest#*"$needle"}
                # The path must end here, so "A" doesn't match "AB".
                if [[ -z $after || ${after:0:1} == [\"\'\ ] ]]; then
                    echo "$f"
                    continue 3
                fi
                rest=$after
            done
        done <"$f"
    done
}

# Same text as the app writes, byte for byte.
render_launcher() {
    local name=$1 dir
    dir=$(profile_dir "$name")
    printf '%s\n' \
        "[Desktop Entry]" \
        "Type=Application" \
        "Name=Signal ($name)" \
        "Comment=Private messaging from your desktop (profile: $name)" \
        "Exec=env BAMF_DESKTOP_FILE_HINT=$SNAP_HINT $SIGNAL_BIN \"--user-data-dir=$dir\" %U" \
        "Icon=$SIGNAL_ICON" \
        "Terminal=false" \
        "StartupWMClass=Signal" \
        "Categories=Network;InstantMessaging;Chat;" \
        "X-SnapInstanceName=signal-desktop" \
        "X-SnapAppName=signal-desktop" \
        "X-MultiSignal-Profile=$name"
}

# Writes Signal-<name>.desktop atomically, mode 0644. Refuses paths the
# desktop-entry spec would need escaped; real home folders never have them.
write_launcher() {
    local name=$1 dir tmp
    dir=$(profile_dir "$name")
    if [[ $dir == *[\"\`\$\\%]* || $dir == *$'\n'* ]]; then
        echo "cannot write a launcher for the path $dir" >&2
        return 1
    fi
    mkdir -p "$APPLICATIONS" || return 1
    tmp=$(mktemp "$APPLICATIONS/.Signal-$name.XXXXXX") || return 1
    if render_launcher "$name" >"$tmp" && chmod 644 "$tmp" && mv -f "$tmp" "$(own_launcher "$name")"; then
        return 0
    fi
    rm -f "$tmp"
    return 1
}

join() {
    local IFS=,
    local s="$*"
    echo "${s//,/, }"
}

require_profile() {
    local name=$1
    if ! valid_name "$name" || [[ ! -d $(profile_dir "$name") ]]; then
        die "There is no profile named “$name”. See: multisignal.sh list"
    fi
}

cmd_list() {
    [[ $# -eq 0 ]] || { usage >&2; exit 2; }
    local names name dirs status size menu launchers files
    names=$(list_names)
    if [[ -z $names ]]; then
        echo "No profiles yet. Create one with: multisignal.sh create NAME"
        return
    fi
    dirs=$(running_dirs)
    printf '%-24s %-8s %7s  %s\n' NAME STATUS SIZE "APP MENU ENTRY"
    while IFS= read -r name; do
        status=-
        grep -qxF -- "$(profile_dir "$name")" <<<"$dirs" && status=running
        size=$(du -sh "$(profile_dir "$name")" 2>/dev/null | cut -f1)
        launchers=$(find_launchers "$name")
        if [[ -z $launchers ]]; then
            menu=missing
        else
            files=()
            while IFS= read -r f; do files+=("${f##*/}"); done <<<"$launchers"
            menu=$(join "${files[@]}")
        fi
        printf '%-24s %-8s %7s  %s\n' "$name" "$status" "$size" "$menu"
    done <<<"$names"
}

cmd_create() {
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    local name dir fix existing
    name=$(trim "$1")
    if ! valid_name "$name"; then
        fix=$(suggest "$name")
        if [[ -n $fix ]]; then
            die "“$name” isn't a valid profile name. Try “$fix”."
        fi
        die "“$name” isn't a valid profile name. Use letters, digits, dot, underscore and dash."
    fi
    existing=$(existing_ignoring_case "$name")
    [[ -z $existing ]] || die "A profile named “$existing” already exists"
    dir=$(profile_dir "$name")
    mkdir -p "$SIGNAL_BASE"
    mkdir "$dir" 2>/dev/null || die "A profile named “$name” already exists"
    if ! write_launcher "$name"; then
        rmdir "$dir" # new and empty, so nothing is lost
        die "Could not write the app menu entry for “$name”"
    fi
    echo "Created “$name”. Open it from your app menu as “Signal ($name)” or with: multisignal.sh open $name"
}

cmd_open() {
    [[ $# -eq 1 || $# -eq 2 ]] || { usage >&2; exit 2; }
    local name=$1 link=${2:-}
    require_profile "$name"
    if [[ -n $link && ! ${link,,} =~ ^(sgnl|signalcaptcha):// ]]; then
        die "Not a Signal link: $link"
    fi
    local args=("--user-data-dir=$(profile_dir "$name")")
    [[ -z $link ]] || args+=("$link")
    # setsid -f: Signal outlives this script and never becomes its child.
    BAMF_DESKTOP_FILE_HINT=$SNAP_HINT setsid -f "$SIGNAL_BIN" "${args[@]}" </dev/null >/dev/null 2>&1 ||
        die "Could not start Signal"
    echo "Opened Signal ($name)"
}

cmd_delete() {
    local yes=false
    if [[ ${1:-} == --yes || ${1:-} == -y ]]; then
        yes=true
        shift
    fi
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    local name=$1 dir size answer f
    require_profile "$name"
    dir=$(profile_dir "$name")
    if is_running "$name"; then
        die "Quit Signal ($name) first. Closing its window can leave it running in the tray."
    fi
    if ! $yes; then
        [[ -t 0 ]] || die "Add --yes to move “$name” to the Trash (there's no terminal to ask on)"
        size=$(du -sh "$dir" 2>/dev/null | cut -f1)
        read -r -p "Move “$name” ($size) and its app menu entry to the Trash? [y/N] " answer
        if [[ $answer != [yY]* ]]; then
            echo "Nothing was moved."
            exit 1
        fi
    fi
    # Menu entries first: if the data then can't be moved, what's left is data
    # without an entry (fixable with repair), never an entry without data.
    while IFS= read -r f; do
        [[ -z $f ]] || gio trash -- "$f" || die "Could not move ${f##*/} to the Trash"
    done < <(find_launchers "$name")
    gio trash -- "$dir" || die "Could not move $dir to the Trash"
    echo "Moved “$name” to the Trash. You can restore it from the Trash until it's emptied."
}

cmd_repair() {
    [[ $# -eq 0 ]] || { usage >&2; exit 2; }
    local name first updated=() created=() skipped=()
    while IFS= read -r name; do
        [[ -n $name ]] || continue
        if [[ -e $(own_launcher "$name") ]]; then
            write_launcher "$name" || die "Could not rewrite the entry for “$name”"
            updated+=("$name")
        elif first=$(find_launchers "$name" | head -n 1) && [[ -n $first ]]; then
            skipped+=("$name (${first##*/})")
        else
            write_launcher "$name" || die "Could not write the entry for “$name”"
            created+=("$name")
        fi
    done < <(list_names)
    if [[ ${#updated[@]} -eq 0 && ${#created[@]} -eq 0 && ${#skipped[@]} -eq 0 ]]; then
        echo "No profiles to repair."
        return
    fi
    [[ ${#updated[@]} -eq 0 ]] || echo "Updated: $(join "${updated[@]}")"
    [[ ${#created[@]} -eq 0 ]] || echo "Created: $(join "${created[@]}")"
    [[ ${#skipped[@]} -eq 0 ]] || echo "Left alone (made by hand): $(join "${skipped[@]}")"
}

main() {
    local command=${1:-}
    [[ $# -eq 0 ]] || shift
    case $command in
        list) cmd_list "$@" ;;
        create) cmd_create "$@" ;;
        open) cmd_open "$@" ;;
        delete) cmd_delete "$@" ;;
        repair) cmd_repair "$@" ;;
        help | -h | --help) usage ;;
        "")
            usage >&2
            exit 2
            ;;
        *)
            echo "multisignal.sh: unknown command: $command" >&2
            usage >&2
            exit 2
            ;;
    esac
}

main "$@"
