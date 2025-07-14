#!/bin/bash

set -e

# Check if pkexec is available
if ! command -v pkexec &>/dev/null; then
    zenity --error --text="pkexec is required but not installed.\nInstall it with:\nsudo apt install policykit-1"
    exit 1
fi

NEED_PRIVILEGE=false

# Determine if we need to install signal-desktop
if ! snap list | grep -q "^signal-desktop"; then
    NEED_PRIVILEGE=true
fi

# If we need to escalate privileges, ask the user
if $NEED_PRIVILEGE; then
    zenity --question --title="Administrator Access Required" \
        --text="This script needs to install required software.\n\nClick OK to continue and enter your password when prompted." \
        --width=400

    if [[ $? -ne 0 ]]; then
        zenity --info --text="Installation cancelled."
        exit 0
    fi

    (
        echo "10"
        echo "# Checking snap and apt..."
        sleep 0.5

        echo "30"
        echo "# Installing Signal Desktop if needed..."
        sleep 0.5

        pkexec bash -c '
            set -e
            if ! snap list | grep -q "^signal-desktop"; then
                snap install signal-desktop
            fi
        '

        echo "100"
        echo "# Installation complete."
        sleep 0.5
    ) | zenity --progress --title="Installing Requirements" \
               --text="Preparing to install..." \
               --percentage=0 --auto-close --width=400
fi

# Ask user to create or delete a profile
ACTION=$(zenity --list --radiolist \
    --title="Signal Profile Manager" \
    --text="Choose an action:" \
    --column="Select" --column="Action" \
    TRUE "Create New Profile" FALSE "Delete Existing Profile" \
    --width=400 --height=200)

if [[ -z "$ACTION" ]]; then
    exit 0
fi

APPLICATIONS_DIR="$HOME/.local/share/applications"
SIGNAL_BASE_DIR="$HOME/Signal"

if [[ "$ACTION" == "Create New Profile" ]]; then
    PROFILE_NAME=$(zenity --entry --title="Create New Signal Profile" \
        --text="Enter a name for the new Signal profile:")

    if [[ -z "$PROFILE_NAME" ]]; then
        zenity --error --text="Profile name cannot be empty or cancelled."
        exit 1
    fi

    PROFILE_DIR="$SIGNAL_BASE_DIR/$PROFILE_NAME"
    NEW_DESKTOP_FILE="$APPLICATIONS_DIR/Signal-$PROFILE_NAME.desktop"

    mkdir -p "$APPLICATIONS_DIR" "$PROFILE_DIR"

    cat > "$NEW_DESKTOP_FILE" <<EOL
[Desktop Entry]
X-SnapInstanceName=signal-desktop
Name=Signal ($PROFILE_NAME)
X-SnapAppName=signal-desktop
Exec=sh -c 'env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop /snap/bin/signal-desktop --user-data-dir=$HOME/Signal/$PROFILE_NAME %U'
Terminal=false
Type=Application
Icon=/snap/signal-desktop/current/meta/gui/signal-desktop.png
StartupWMClass=Signal
Comment=Private messaging from your desktop
MimeType=x-scheme-handler/sgnl;x-scheme-handler/signalcaptcha;
Categories=Network;InstantMessaging;Chat;
EOL

    chmod +x "$NEW_DESKTOP_FILE"

    zenity --info --text="Signal profile created:\n$NEW_DESKTOP_FILE"

elif [[ "$ACTION" == "Delete Existing Profile" ]]; then
    # Get list of profiles
    mapfile -t PROFILE_FILES < <(find "$APPLICATIONS_DIR" -maxdepth 1 -name 'Signal-*.desktop' -printf '%f\n')
    if [[ ${#PROFILE_FILES[@]} -eq 0 ]]; then
        zenity --info --text="No Signal profiles found to delete."
        exit 0
    fi

    # Build zenity checklist input
    ZENITY_INPUT=()
    for FILE in "${PROFILE_FILES[@]}"; do
        PROFILE_NAME="${FILE#Signal-}"
        PROFILE_NAME="${PROFILE_NAME%.desktop}"
        ZENITY_INPUT+=("FALSE" "$PROFILE_NAME")
    done

    SELECTED=$(zenity --list --checklist \
        --title="Delete Signal Profiles" \
        --text="Select one or more profiles to delete:" \
        --column="Select" --column="Profile" \
        "${ZENITY_INPUT[@]}" \
        --width=400 --height=400)

    if [[ -z "$SELECTED" ]]; then
        zenity --info --text="No profiles selected. Deletion cancelled."
        exit 0
    fi

    IFS="|" read -ra PROFILES_TO_DELETE <<< "$SELECTED"

    CONFIRM_TEXT="The following profiles will be deleted:\n"
    for P in "${PROFILES_TO_DELETE[@]}"; do
        CONFIRM_TEXT+="- $P\n"
    done

    zenity --question --title="Confirm Deletion" \
        --text="$CONFIRM_TEXT\nThis will remove both launchers and data directories."

    if [[ $? -ne 0 ]]; then
        zenity --info --text="Deletion cancelled."
        exit 0
    fi

    # Perform deletion
    for PROFILE in "${PROFILES_TO_DELETE[@]}"; do
        DESKTOP_FILE="$APPLICATIONS_DIR/Signal-$PROFILE.desktop"
        PROFILE_DIR="$SIGNAL_BASE_DIR/$PROFILE"
        rm -f "$DESKTOP_FILE"
        rm -rf "$PROFILE_DIR"
    done

    zenity --info --text="Selected profiles have been deleted."
fi
