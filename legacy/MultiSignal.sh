#!/bin/bash
# MultiSignal — run several Signal Desktop (snap) profiles side by side.
#
# Each profile is a data directory under ~/Signal/<name> plus a launcher in
# ~/.local/share/applications that starts Signal with --user-data-dir.
# Deleting moves data and launchers to the Trash, so it can be undone.

# No `set -e`: zenity returns non-zero for Cancel, and every dialog result is
# checked explicitly below.
set -uo pipefail

readonly APPLICATIONS_DIR="$HOME/.local/share/applications"
readonly SIGNAL_BASE_DIR="$HOME/Signal"
readonly SIGNAL_BIN="${SIGNAL_BIN:-/snap/bin/signal-desktop}"
readonly SNAP_DESKTOP_HINT=/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop
readonly SIGNAL_ICON=/snap/signal-desktop/current/meta/gui/signal-desktop.png
readonly TITLE="Signal Profile Manager"
readonly WIDTH=460

# ---------------------------------------------------------------- dialogs ---

# zenity lists have a fixed height and GTK4 hides the scrollbar until you
# scroll, so rows below the fold are easy to miss. Size each list to show all
# rows: ~206 px of title, prompt, column header and buttons, plus one row
# height per row (33 px plain, 42 px with checkboxes). Capped for small
# screens, where the list then has to scroll.
list_height() {
    local rows=$1 row_px=$2 h
    h=$(( 206 + rows * row_px ))
    (( h > 640 )) && h=640
    printf '%s' "$h"
}

info()  { zenity --info  --title="$TITLE" --width="$WIDTH" --text="$1"; }
error() { zenity --error --title="$TITLE" --width="$WIDTH" --text="$1"; }

# Escapes text for zenity's Pango markup (profile names can contain none of
# these, but launcher paths and error output can).
markup_escape() {
    local s=$1
    s=${s//&/&amp;}; s=${s//</&lt;}; s=${s//>/&gt;}
    printf '%s' "$s"
}

# ---------------------------------------------------------------- profiles --

# Letters, digits, dot, underscore, dash; must start with a letter or digit.
# This rules out empty names, ".", "..", "/", spaces, quotes and "|", all of
# which previously broke launchers or made deletion hit the wrong directory.
valid_profile_name() {
    [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]]
}

# Turns a rejected name like "My Work!" into "My-Work", so the entry box can
# come back with something the user only has to confirm.
suggest_profile_name() {
    local s=$1
    s=${s//[[:space:]]/-}
    s=${s//[^A-Za-z0-9._-]/}
    while [[ $s == *--* ]]; do s=${s//--/-}; done
    s=${s#"${s%%[A-Za-z0-9]*}"}    # must start with a letter or digit
    printf '%s' "${s:0:64}"
}

# "A, B, C" for dialog text.
join_names() {
    local out
    out=$(printf '%s, ' "$@")
    printf '%s' "${out%, }"
}

profile_dir() { printf '%s/%s' "$SIGNAL_BASE_DIR" "$1"; }
own_launcher() { printf '%s/Signal-%s.desktop' "$APPLICATIONS_DIR" "$1"; }

# Profiles are the data directories, not the launchers, so profiles whose
# launcher is missing or has a different file name are still found.
list_profiles() {
    local dir name
    [[ -d $SIGNAL_BASE_DIR ]] || return 0
    for dir in "$SIGNAL_BASE_DIR"/*/; do
        [[ -d $dir ]] || continue
        name=$(basename "$dir")
        valid_profile_name "$name" && printf '%s\n' "$name"
    done
}

# Prints the existing profile whose name matches $1 ignoring case, so "work"
# and "Work" can't both exist as near-identical app menu entries.
existing_profile_named() {
    local p
    while IFS= read -r p; do
        [[ ${p,,} == "${1,,}" ]] && { printf '%s' "$p"; return 0; }
    done < <(list_profiles)
    return 1
}

# Every launcher (ours or hand-made) that starts Signal with this profile.
find_launchers() {
    local dir
    dir=$(profile_dir "$1")
    [[ -d $APPLICATIONS_DIR ]] || return 0
    grep -lF \
        -e "--user-data-dir=$dir\"" \
        -e "--user-data-dir=$dir " \
        -e "--user-data-dir=$dir'" \
        -- "$APPLICATIONS_DIR"/*.desktop 2>/dev/null || true
}

regex_escape() { sed 's/[][\.*^$+?(){}|/]/\\&/g' <<<"$1"; }

profile_running() {
    local dir
    dir=$(regex_escape "$(profile_dir "$1")")
    pgrep -f -- "signal-desktop.* --user-data-dir=$dir( |\$)" >/dev/null
}

profile_size() { du -sh -- "$(profile_dir "$1")" 2>/dev/null | cut -f1; }

profile_status() {
    # Blank unless running, so running profiles stand out.
    if profile_running "$1"; then echo "Running"; else echo ""; fi
}

write_launcher() {
    local name=$1 file tmp
    file=$(own_launcher "$name")
    mkdir -p "$APPLICATIONS_DIR" || return 1
    tmp=$(mktemp "$APPLICATIONS_DIR/.Signal-$name.XXXXXX") || return 1
    # Exec uses a quoted argument instead of `sh -c '…'`, which is invalid per
    # the desktop entry spec. No MimeType: sgnl:// links keep opening in the
    # main Signal instead of whichever profile launcher was written last.
    cat >"$tmp" <<EOL
[Desktop Entry]
Type=Application
Name=Signal ($name)
Comment=Private messaging from your desktop (profile: $name)
Exec=env BAMF_DESKTOP_FILE_HINT=$SNAP_DESKTOP_HINT $SIGNAL_BIN "--user-data-dir=$(profile_dir "$name")" %U
Icon=$SIGNAL_ICON
Terminal=false
StartupWMClass=Signal
Categories=Network;InstantMessaging;Chat;
X-SnapInstanceName=signal-desktop
X-SnapAppName=signal-desktop
X-MultiSignal-Profile=$name
EOL
    # mktemp creates files as 0600; launchers are normally world-readable.
    chmod 644 -- "$tmp" && mv -f -- "$tmp" "$file"
}

launch_profile() {
    setsid -f env BAMF_DESKTOP_FILE_HINT="$SNAP_DESKTOP_HINT" \
        "$SIGNAL_BIN" "--user-data-dir=$(profile_dir "$1")" >/dev/null 2>&1
}

# Prints the chosen profile, or nothing if cancelled / no profiles exist.
pick_profile() {
    local prompt=$1 name rows=()
    while IFS= read -r name; do
        rows+=("$name" "$(profile_size "$name")" "$(profile_status "$name")")
    done < <(list_profiles)
    if (( ${#rows[@]} == 0 )); then
        info "No Signal profiles yet. Choose <b>Create profile</b> first."
        return 0
    fi
    zenity --list --title="$TITLE" --text="$prompt" --ok-label="Open" \
        --column="Profile" --column="Size" --column="Status" \
        "${rows[@]}" --width="$WIDTH" --height="$(list_height $(( ${#rows[@]} / 3 )) 33)" || true
}

# ----------------------------------------------------------------- actions --

action_create() {
    local name="" dir existing suggestion
    while true; do
        # No literal "_" in this prompt: zenity shows the entry label with
        # mnemonics, so "Family_2" would be displayed as "Family2".
        name=$(zenity --entry --title="$TITLE" --width="$WIDTH" \
            --text="Name for the new profile.\nLetters, digits, dot, underscore and dash only (e.g. Work, Family-2)." \
            --entry-text="$name") || return 0
        # Trim surrounding whitespace.
        name="${name#"${name%%[![:space:]]*}"}"
        name="${name%"${name##*[![:space:]]}"}"
        if ! valid_profile_name "$name"; then
            suggestion=$(suggest_profile_name "$name")
            if valid_profile_name "$suggestion"; then
                error "“$(markup_escape "$name")” isn't a valid profile name: spaces and symbols aren't allowed.\n\nTry “$suggestion”."
                name=$suggestion
            else
                error "“$(markup_escape "$name")” isn't a valid profile name.\n\nUse letters, digits, dot, underscore and dash, starting with a letter or digit."
            fi
            continue
        fi
        dir=$(profile_dir "$name")
        if existing=$(existing_profile_named "$name") || [[ -e $dir ]]; then
            error "A profile named <b>${existing:-$name}</b> already exists.\n\nIf its launcher is missing, use <b>Repair launchers</b>."
            continue
        fi
        break
    done

    if ! mkdir -p -- "$dir" || ! write_launcher "$name"; then
        error "Could not create profile <b>$name</b>.\nCheck that $(markup_escape "$SIGNAL_BASE_DIR") and $(markup_escape "$APPLICATIONS_DIR") are writable."
        return 1
    fi

    if zenity --question --title="$TITLE" --width="$WIDTH" --icon=emblem-ok-symbolic \
        --text="Profile <b>$name</b> created.\n\nIt appears in your app menu as “Signal ($name)”." \
        --ok-label="Launch now" --cancel-label="Close"; then
        launch_profile "$name"
        exit 0      # Signal is open; the manager has done its job.
    fi
}

action_launch() {
    local name
    name=$(pick_profile "Choose a profile to open:")
    [[ -n $name ]] || return 0
    valid_profile_name "$name" || return 1
    launch_profile "$name"
    exit 0          # Signal is open; the manager has done its job.
}

action_delete() {
    local name rows=() selected=() running=() launchers=() targets=()
    local text

    while IFS= read -r name; do
        rows+=(FALSE "$name" "$(profile_size "$name")" "$(profile_status "$name")")
    done < <(list_profiles)
    if (( ${#rows[@]} == 0 )); then
        info "No Signal profiles found."
        return 0
    fi

    mapfile -t selected < <(zenity --list --checklist --title="$TITLE" \
        --text="Tick the profiles to delete. Nothing is deleted until you confirm." \
        --column="" --column="Profile" --column="Size" --column="Status" \
        --ok-label="Continue…" \
        --separator=$'\n' "${rows[@]}" --width="$WIDTH" \
        --height="$(list_height $(( ${#rows[@]} / 4 )) 42)")
    # Drop empty lines and anything that isn't a real profile (defence in
    # depth: an empty name here would otherwise target ~/Signal itself).
    local chosen=()
    for name in "${selected[@]}"; do
        valid_profile_name "$name" && [[ -d $(profile_dir "$name") ]] && chosen+=("$name")
    done
    (( ${#chosen[@]} > 0 )) || return 0

    for name in "${chosen[@]}"; do
        profile_running "$name" && running+=("$name")
    done
    if (( ${#running[@]} > 0 )); then
        # Closing Signal's window can leave it running in the tray, so ask
        # the user to quit it rather than close it.
        local windows=()
        for name in "${running[@]}"; do windows+=("Signal ($name)"); done
        if (( ${#running[@]} == ${#chosen[@]} )); then
            error "Quit $(join_names "${windows[@]}") first. Closing the window can leave Signal running in the tray.\n\nNothing was deleted."
            return 0
        fi
        local verb="is" pronoun="it"
        (( ${#running[@]} > 1 )) && { verb="are"; pronoun="them"; }
        zenity --question --title="$TITLE" --width="$WIDTH" \
            --ok-label="Skip and continue" --cancel-label="Cancel" \
            --text="$(join_names "${running[@]}") $verb still running and will be skipped. To delete $pronoun too, quit $(join_names "${windows[@]}") first.\n\nContinue with the other profiles?" \
            || return 0
        local keep=()
        for name in "${chosen[@]}"; do
            [[ " ${running[*]} " == *" $name "* ]] || keep+=("$name")
        done
        chosen=("${keep[@]}")
    fi

    text="These profiles will be moved to the Trash, including their messages and launchers:\n\n"
    for name in "${chosen[@]}"; do
        text+="• <b>$name</b> ($(profile_size "$name"))\n"
    done
    text+="\nYou can restore them from the Trash until it is emptied."
    zenity --question --title="$TITLE" --width="$WIDTH" --icon=dialog-warning \
        --default-cancel --ok-label="Move to Trash" --cancel-label="Cancel" \
        --text="$text" || return 0

    for name in "${chosen[@]}"; do
        mapfile -t launchers < <(find_launchers "$name")
        targets=("$(profile_dir "$name")" "${launchers[@]}")
        local err
        if ! err=$(gio trash -- "${targets[@]}" 2>&1); then
            error "Could not move <b>$name</b> to the Trash:\n\n$(markup_escape "$err")\n\nNothing else was deleted."
            return 1
        fi
    done
    info "Moved to Trash: $(join_names "${chosen[@]}")"
}

# Rewrites this tool's launchers in the current format and creates launchers
# for profiles that have none. Hand-made launchers are left untouched.
action_repair() {
    local name own updated=() created=() skipped=() launchers
    while IFS= read -r name; do
        own=$(own_launcher "$name")
        launchers=$(find_launchers "$name")
        if [[ -e $own ]]; then
            write_launcher "$name" && updated+=("$name")
        elif [[ -z $launchers ]]; then
            write_launcher "$name" && created+=("$name")
        else
            skipped+=("$name ($(basename "${launchers%%$'\n'*}"))")
        fi
    done < <(list_profiles)

    local text=""
    (( ${#updated[@]} )) && text+="Updated: $(join_names "${updated[@]}")\n"
    (( ${#created[@]} )) && text+="Created: $(join_names "${created[@]}")\n"
    (( ${#skipped[@]} )) && text+="Left alone (own launcher): $(join_names "${skipped[@]}")\n"
    [[ -n $text ]] || text="No profiles found."
    info "$(markup_escape "$text")"
}

# ------------------------------------------------------------ installation --

signal_installed() { snap list signal-desktop >/dev/null 2>&1; }

other_signal_install() {
    if command -v flatpak >/dev/null 2>&1 && flatpak info org.signal.Signal >/dev/null 2>&1; then
        echo "Flatpak"
    elif [[ -x /opt/Signal/signal-desktop ]]; then
        echo "apt"
    fi
}

ensure_signal_installed() {
    signal_installed && return 0

    local other text
    other=$(other_signal_install)
    text="Signal Desktop (snap) isn't installed.\n\n"
    [[ -n $other ]] && text+="You have the $other version, but this tool manages the snap version.\n\n"
    text+="Install it now? You'll be asked for your password."
    zenity --question --title="$TITLE" --width="$WIDTH" --text="$text" \
        --ok-label="Install" --cancel-label="Quit" || return 1

    if ! command -v pkexec >/dev/null 2>&1; then
        error "pkexec isn't available. Install Signal from a terminal instead:\n\n<tt>sudo snap install signal-desktop</tt>"
        return 1
    fi

    local errfile rc
    errfile=$(mktemp)
    pkexec snap install signal-desktop >/dev/null 2>"$errfile" |
        zenity --progress --pulsate --auto-close --no-cancel --title="$TITLE" \
            --text="Installing Signal Desktop…" --width="$WIDTH"
    rc=${PIPESTATUS[0]}

    if (( rc == 126 || rc == 127 )) && ! signal_installed; then
        rm -f "$errfile"
        error "Installation cancelled (no password entered)."
        return 1
    fi
    if (( rc != 0 )) || ! signal_installed; then
        error "Installing Signal Desktop failed (exit code $rc):\n\n$(markup_escape "$(tail -n 5 "$errfile")")"
        rm -f "$errfile"
        return 1
    fi
    rm -f "$errfile"
}

# -------------------------------------------------------------------- main --

main() {
    if ! command -v zenity >/dev/null 2>&1; then
        echo "MultiSignal needs zenity: sudo apt install zenity" >&2
        exit 1
    fi
    ensure_signal_installed || exit 1

    local action actions=("Launch profile" "Create profile" "Delete profiles" "Repair launchers")
    while true; do
        action=$(zenity --list --title="$TITLE" --text="What do you want to do?" \
            --column="Action" "${actions[@]}" \
            --width="$WIDTH" --height="$(list_height ${#actions[@]} 33)") || exit 0
        case $action in
            "Launch profile")   action_launch ;;
            "Create profile")   action_create ;;
            "Delete profiles")  action_delete ;;
            "Repair launchers") action_repair ;;
        esac
    done
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    main "$@"
fi
