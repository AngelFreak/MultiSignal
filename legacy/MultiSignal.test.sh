#!/bin/bash
# Tests for MultiSignal.sh. Runs the real script end to end in a throwaway
# $HOME, with stub zenity/snap/pkexec/gio/signal-desktop on PATH.
#
#   bash MultiSignal.test.sh
#
# Stub zenity answers --info/--error/--progress with OK automatically; every
# other dialog pops the next "exit_code|stdout" line from $ZENITY_QUEUE, and an
# empty queue means Cancel (so the main menu loop ends).

set -uo pipefail

SCRIPT="$(cd "$(dirname "$0")" && pwd)/MultiSignal.sh"
ORIG_PATH=$PATH
RESULTS=$(mktemp)
trap 'rm -f "$RESULTS"' EXIT

# ------------------------------------------------------------------ harness --

setup() {
    T=$(mktemp -d)
    export HOME="$T/home" ZENITY_QUEUE="$T/queue" ZENITY_LOG="$T/zenity.log"
    export STUB_LOG="$T/stub.log" STUB_DIR="$T" SIGNAL_BIN="$T/bin/signal-desktop"
    mkdir -p "$HOME/Signal" "$HOME/.local/share/applications" "$T/bin" "$T/trash"
    : >"$ZENITY_QUEUE"; : >"$ZENITY_LOG"; : >"$STUB_LOG"
    touch "$T/snap_installed"

    cat >"$T/bin/zenity" <<'EOF'
#!/bin/bash
args="$*"
printf '%s\n' "${args//$'\n'/\\n}" >>"$ZENITY_LOG"   # one line per call
case " $* " in
    *" --progress "*) cat >/dev/null; exit 0 ;;
    *" --info "*|*" --error "*) exit 0 ;;
esac
line=$(head -n 1 "$ZENITY_QUEUE")
[[ -n $line ]] || exit 1
sed -i 1d "$ZENITY_QUEUE"
out=${line#*|}
[[ -n $out ]] && printf '%b\n' "$out"
exit "${line%%|*}"
EOF
    cat >"$T/bin/snap" <<'EOF'
#!/bin/bash
echo "snap $*" >>"$STUB_LOG"
case $1 in
    list) [[ -e $STUB_DIR/snap_installed ]] ;;
    install)
        if [[ -e $STUB_DIR/install_fails ]]; then echo "error: boom" >&2; exit 1; fi
        touch "$STUB_DIR/snap_installed" ;;
esac
EOF
    cat >"$T/bin/pkexec" <<'EOF'
#!/bin/bash
echo "pkexec $*" >>"$STUB_LOG"
[[ -e $STUB_DIR/pkexec_rc ]] && exit "$(cat "$STUB_DIR/pkexec_rc")"
exec "$@"
EOF
    cat >"$T/bin/gio" <<'EOF'
#!/bin/bash
echo "gio $*" >>"$STUB_LOG"
[[ $1 == trash ]] || exit 1
shift; [[ $1 == -- ]] && shift
for p in "$@"; do mv -- "$p" "$STUB_DIR/trash/" || exit 1; done
EOF
    cat >"$T/bin/signal-desktop" <<'EOF'
#!/bin/bash
echo "signal $*" >>"$STUB_LOG"
EOF
    cat >"$T/bin/flatpak" <<'EOF'
#!/bin/bash
exit 1
EOF
    chmod +x "$T/bin/"*
    export PATH="$T/bin:$ORIG_PATH"
}

teardown() {
    [[ -n ${FAKE_PID:-} ]] && kill "$FAKE_PID" 2>/dev/null
    FAKE_PID=""
    rm -rf "$T"
}

answer() { printf '%s|%s\n' "$1" "${2:-}" >>"$ZENITY_QUEUE"; }

run_script() {
    timeout 20 bash "$SCRIPT" </dev/null >"$T/out" 2>&1
    RC=$?
}

# A launcher in the old format the previous script version wrote.
legacy_launcher() {
    cat >"$HOME/.local/share/applications/Signal-$1.desktop" <<EOF
[Desktop Entry]
Name=Signal ($1)
Exec=sh -c 'env BAMF_DESKTOP_FILE_HINT=x /snap/bin/signal-desktop --user-data-dir=$HOME/Signal/$1 %U'
Type=Application
EOF
}

# Waits up to 2 s for setsid-launched stubs to write their log line.
wait_for_log() {
    local i
    for i in $(seq 20); do grep -qF -- "$1" "$STUB_LOG" && return 0; sleep 0.1; done
    return 1
}

check() {
    local desc=$1; shift
    if "$@"; then
        echo PASS >>"$RESULTS"
    else
        echo "FAIL $CURRENT: $desc" >>"$RESULTS"
        echo "  FAIL: $desc"
    fi
}

run_test() {
    CURRENT=$1
    echo "- $1"
    # Subshell: tests source the script (readonly vars) and change HOME/PATH.
    ( setup; "$1"; teardown )
}

launcher() { printf '%s/.local/share/applications/Signal-%s.desktop' "$HOME" "$1"; }
not() { ! "$@"; }

# -------------------------------------------------------------------- unit --

test_profile_name_validation() {
    # shellcheck source=MultiSignal.sh
    source "$SCRIPT"
    local n
    for n in Work UKR Family_2 a.b-c 9lives; do
        check "accepts '$n'" valid_profile_name "$n"
    done
    for n in "" . .. ../x a/b "My Profile" "it's" "a|b" -rf .hidden " "; do
        check "rejects '$n'" not valid_profile_name "$n"
    done
}

test_launcher_is_valid_desktop_entry() {
    source "$SCRIPT"
    write_launcher Work
    check "desktop-file-validate passes" desktop-file-validate "$(launcher Work)"
    check "Exec quotes the data dir" \
        grep -qF "\"--user-data-dir=$HOME/Signal/Work\" %U" "$(launcher Work)"
    check "no MimeType (sgnl:// stays with main Signal)" not grep -q '^MimeType' "$(launcher Work)"
}

# ------------------------------------------------------------- integration --

test_menu_cancel_exits_cleanly() {
    run_script
    check "exit code 0" test "$RC" -eq 0
    check "no install prompt when snap installed" not grep -q "isn't installed" "$ZENITY_LOG"
}

test_create_profile() {
    answer 0 "Create profile"
    answer 0 "  Work  "
    answer 1            # "Close" instead of "Launch now"
    run_script
    check "exit code 0" test "$RC" -eq 0
    check "data dir created (name trimmed)" test -d "$HOME/Signal/Work"
    check "launcher created" test -f "$(launcher Work)"
    check "launcher validates" desktop-file-validate "$(launcher Work)"
    check "Signal not launched" not grep -q '^signal' "$STUB_LOG"
    check "returned to main menu" test "$(grep -c 'What do you want' "$ZENITY_LOG")" -eq 2
}

test_create_then_launch_now() {
    answer 0 "Create profile"
    answer 0 "Work"
    answer 0            # "Launch now"
    run_script
    check "Signal launched with the profile dir" \
        wait_for_log "signal --user-data-dir=$HOME/Signal/Work"
    check "manager closes after launching" test "$(grep -c 'What do you want' "$ZENITY_LOG")" -eq 1
    check "success dialog uses a success icon, not a question mark" \
        grep -q -- "--question.*--icon=emblem-ok-symbolic.*Profile <b>Work</b> created" "$ZENITY_LOG"
}

test_create_suggests_valid_name() {
    answer 0 "Create profile"
    answer 0 "My Work"
    answer 1
    run_script
    check "error suggests My-Work" grep -q "Try “My-Work”" "$ZENITY_LOG"
    check "entry is prefilled with My-Work" grep -q -- "--entry-text=My-Work" "$ZENITY_LOG"
}

test_create_prompt_has_no_underscore() {
    # zenity treats "_" in the entry prompt as a mnemonic and hides it, so
    # an example like "Family_2" was shown as "Family2".
    answer 0 "Create profile"
    answer 1
    run_script
    check "entry prompt contains no literal underscore" \
        not grep -q -- "--entry .*--text=[^-]*_" "$ZENITY_LOG"
}

test_create_rejects_case_insensitive_duplicate() {
    mkdir -p "$HOME/Signal/Work"
    answer 0 "Create profile"
    answer 0 "work"
    answer 1
    run_script
    check "duplicate error names the existing profile" grep -q "<b>Work</b> already exists" "$ZENITY_LOG"
    check "no second profile created" not test -e "$HOME/Signal/work"
}

test_create_rejects_bad_names() {
    local n
    answer 0 "Create profile"
    for n in "../evil" "My Profile" "a|b" "."; do answer 0 "$n"; done
    answer 1            # cancel the entry dialog
    run_script
    check "4 validation errors shown" test "$(grep -c "isn't a valid profile name" "$ZENITY_LOG")" -eq 4
    check "nothing created in ~/Signal" test -z "$(ls -A "$HOME/Signal")"
    check "nothing created outside ~/Signal" not test -e "$HOME/evil"
    check "no launchers written" test -z "$(ls -A "$HOME/.local/share/applications")"
}

test_create_rejects_duplicate() {
    mkdir -p "$HOME/Signal/Work/keep"
    answer 0 "Create profile"
    answer 0 "Work"
    answer 1
    run_script
    check "duplicate error shown" grep -q "already exists" "$ZENITY_LOG"
    check "existing data untouched" test -d "$HOME/Signal/Work/keep"
    check "no launcher written" not test -e "$(launcher Work)"
}

delete_fixture() {
    mkdir -p "$HOME/Signal/A/data" "$HOME/Signal/B/data"
    legacy_launcher A
    printf '[Desktop Entry]\nName=B\nExec=env X=1 /snap/bin/signal-desktop --user-data-dir=%s %%U\nType=Application\n' \
        "$HOME/Signal/B" >"$HOME/.local/share/applications/custom-b.desktop"
    # A stray launcher with an empty profile name made the old script run
    # `rm -rf ~/Signal/`.
    touch "$HOME/.local/share/applications/Signal-.desktop"
}

test_delete_moves_profile_to_trash() {
    delete_fixture
    answer 0 "Delete profiles"
    answer 0 "A"
    answer 0            # confirm
    run_script
    check "A data moved to Trash" test -d "$T/trash/A/data"
    check "A launcher moved to Trash" test -f "$T/trash/Signal-A.desktop"
    check "A gone" not test -e "$HOME/Signal/A"
    check "B untouched" test -d "$HOME/Signal/B/data"
    check "B launcher untouched" test -f "$HOME/.local/share/applications/custom-b.desktop"
    check "confirmation defaults to Cancel" grep -q -- "--default-cancel" "$ZENITY_LOG"
    local picker
    picker=$(grep -- "--checklist" "$ZENITY_LOG")
    check "picker button says Continue…" grep -qF -- "--ok-label=Continue…" <<<"$picker"
    check "picker says nothing is deleted yet" grep -q "Nothing is deleted until you confirm" <<<"$picker"
    check "stopped profiles have blank status" not grep -q "—" <<<"$picker"
}

# GTK4 hides the scrollbar until you scroll, so lists must be tall enough to
# show every row. Heights must grow with the number of profiles.
logged_height() { grep -- "$1" "$ZENITY_LOG" | head -1 | grep -o -- '--height=[0-9]*' | cut -d= -f2; }

test_lists_grow_with_profile_count() {
    local n small big
    mkdir -p "$HOME/Signal/A" "$HOME/Signal/B"
    answer 0 "Delete profiles"; answer 1
    answer 0 "Launch profile";  answer 1
    run_script
    small=$(logged_height "--checklist"); local small_launch; small_launch=$(logged_height "Choose a profile")

    for n in C D E F G; do mkdir -p "$HOME/Signal/$n"; done
    : >"$ZENITY_LOG"
    answer 0 "Delete profiles"; answer 1
    answer 0 "Launch profile";  answer 1
    run_script
    big=$(logged_height "--checklist")
    check "delete list taller for 7 profiles than 2" test "$big" -gt "$small"
    check "launch list taller for 7 profiles than 2" test "$(logged_height "Choose a profile")" -gt "$small_launch"
    check "7-row delete list fits on a laptop screen" test "$big" -le 700
}

test_main_menu_shows_every_action() {
    run_script
    local h; h=$(logged_height "What do you want")
    # 4 actions at 33 px plus ~206 px of title, header and buttons.
    check "main menu tall enough for 4 actions" test "$h" -ge $((206 + 4 * 33))
}

test_delete_finds_hand_made_launcher() {
    delete_fixture
    answer 0 "Delete profiles"
    answer 0 "B"
    answer 0
    run_script
    check "custom launcher moved to Trash" test -f "$T/trash/custom-b.desktop"
    check "A untouched" test -d "$HOME/Signal/A/data"
}

test_delete_ignores_empty_selection() {
    delete_fixture
    answer 0 "Delete profiles"
    answer 0 "\nA"      # blank line + A, as the old "|A" parse produced
    answer 0
    run_script
    check "~/Signal still exists" test -d "$HOME/Signal"
    check "B survives" test -d "$HOME/Signal/B/data"
    check "only A trashed" test "$(ls "$T/trash" | sort | tr '\n' ' ')" = "A Signal-A.desktop "
}

test_delete_cancel_keeps_everything() {
    delete_fixture
    answer 0 "Delete profiles"
    answer 0 "A"
    answer 1            # Cancel at confirmation
    run_script
    check "A untouched" test -d "$HOME/Signal/A/data"
    check "nothing trashed" test -z "$(ls -A "$T/trash")"
}

test_delete_refuses_running_profile() {
    delete_fixture
    bash -c "exec -a 'signal-desktop --user-data-dir=$HOME/Signal/A' sleep 30" &
    FAKE_PID=$!
    sleep 0.2
    answer 0 "Delete profiles"
    answer 0 "A"
    answer 0
    run_script
    check "error says to quit Signal (tray)" grep -q "Quit Signal (A)" "$ZENITY_LOG"
    check "error says nothing was deleted" grep -q "Nothing was deleted" "$ZENITY_LOG"
    check "A untouched" test -d "$HOME/Signal/A/data"
    check "list shows A as Running" grep -q "A [^ ]* Running" "$ZENITY_LOG"
}

test_delete_skips_running_profile_when_asked() {
    delete_fixture
    bash -c "exec -a 'signal-desktop --user-data-dir=$HOME/Signal/A' sleep 30" &
    FAKE_PID=$!
    sleep 0.2
    answer 0 "Delete profiles"
    answer 0 "A\nB"
    answer 0            # "Skip A and continue"
    answer 0            # confirm
    run_script
    check "skip question names A" grep -q -- "--question.*A is still running" "$ZENITY_LOG"
    check "A untouched" test -d "$HOME/Signal/A/data"
    check "B moved to Trash" test -d "$T/trash/B/data"
}

test_delete_lists_names_with_commas() {
    delete_fixture
    answer 0 "Delete profiles"
    answer 0 "A\nB"
    answer 0
    run_script
    check "summary separates names" grep -q "Moved to Trash: A, B" "$ZENITY_LOG"
}

test_launch_profile() {
    mkdir -p "$HOME/Signal/A"
    answer 0 "Launch profile"
    answer 0 "A"
    run_script
    check "Signal launched for A" wait_for_log "signal --user-data-dir=$HOME/Signal/A"
    check "list button says Open" grep -q -- "Choose a profile.*--ok-label=Open" "$ZENITY_LOG"
    check "manager closes after launching" test "$(grep -c 'What do you want' "$ZENITY_LOG")" -eq 1
    check "exit code 0" test "$RC" -eq 0
}

test_repair_launchers() {
    mkdir -p "$HOME/Signal/UKR" "$HOME/Signal/Personal" "$HOME/Signal/Damon"
    legacy_launcher UKR
    printf '[Desktop Entry]\nName=Damon\nExec=env X=1 /snap/bin/signal-desktop --user-data-dir=%s %%U\nType=Application\n' \
        "$HOME/Signal/Damon" >"$HOME/.local/share/applications/signal-desktop-damon.desktop"
    cp "$HOME/.local/share/applications/signal-desktop-damon.desktop" "$T/damon.orig"
    answer 0 "Repair launchers"
    run_script
    check "legacy launcher rewritten and valid" desktop-file-validate "$(launcher UKR)"
    check "missing launcher created" test -f "$(launcher Personal)"
    check "no duplicate launcher for Damon" not test -e "$(launcher Damon)"
    check "hand-made launcher unchanged" cmp -s "$T/damon.orig" \
        "$HOME/.local/share/applications/signal-desktop-damon.desktop"
    check "summary: Personal created" grep -q "Created: Personal" "$ZENITY_LOG"
    check "summary: UKR updated" grep -q "Updated: UKR" "$ZENITY_LOG"
    check "launchers are mode 644" test "$(stat -c %a "$(launcher Personal)")" = 644

    # Running Repair again rewrites both; nothing is "created" any more.
    : >"$ZENITY_LOG"
    answer 0 "Repair launchers"
    run_script
    check "second run: both updated" grep -q "Updated: Personal, UKR" "$ZENITY_LOG"
    check "second run: nothing created" not grep -q "Created:" "$ZENITY_LOG"
}

test_install_declined() {
    rm "$T/snap_installed"
    answer 1            # Quit
    run_script
    check "exit code 1" test "$RC" -eq 1
    check "pkexec not run" not grep -q pkexec "$STUB_LOG"
}

test_install_password_cancelled() {
    rm "$T/snap_installed"
    echo 126 >"$T/pkexec_rc"
    answer 0            # Install
    run_script
    check "exit code 1" test "$RC" -eq 1
    check "cancel message shown" grep -q "Installation cancelled" "$ZENITY_LOG"
    check "main menu not shown" not grep -q "What do you want" "$ZENITY_LOG"
}

test_install_failure_reported() {
    rm "$T/snap_installed"
    touch "$T/install_fails"
    answer 0
    run_script
    check "exit code 1" test "$RC" -eq 1
    check "error includes snap output" grep -q "failed.*boom" "$ZENITY_LOG"
    check "main menu not shown" not grep -q "What do you want" "$ZENITY_LOG"
}

test_install_success_continues_to_menu() {
    rm "$T/snap_installed"
    answer 0
    run_script
    check "snap install ran via pkexec" grep -q "pkexec snap install signal-desktop" "$STUB_LOG"
    check "main menu shown" grep -q "What do you want" "$ZENITY_LOG"
    check "exit code 0" test "$RC" -eq 0
}

# --------------------------------------------------------------------- run --

for t in $(declare -F | awk '{print $3}' | grep '^test_'); do
    run_test "$t"
done
PASS=$(grep -c '^PASS' "$RESULTS")
FAIL=$(grep -c '^FAIL' "$RESULTS")
echo
echo "$PASS passed, $FAIL failed"
(( FAIL == 0 )) || { grep '^FAIL' "$RESULTS" | sed 's/^FAIL /  /'; exit 1; }
