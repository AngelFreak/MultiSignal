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

    # Run privileged commands and show progress
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

# Prompt for profile name
PROFILE_NAME=$(zenity --entry --title="Create New Signal Profile" \
    --text="Enter a name for the new Signal profile:")

if [[ -z "$PROFILE_NAME" ]]; then
    zenity --error --text="Profile name cannot be empty or cancelled."
    exit 1
fi

# Define paths
APPLICATIONS_DIR="$HOME/.local/share/applications"
SIGNAL_BASE_DIR="$HOME/Signal"
PROFILE_DIR="$SIGNAL_BASE_DIR/$PROFILE_NAME"
NEW_DESKTOP_FILE="$APPLICATIONS_DIR/Signal-$PROFILE_NAME.desktop"

# Create directories
mkdir -p "$APPLICATIONS_DIR" "$PROFILE_DIR"

# Create .desktop launcher
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

# Done
zenity --info --text="Signal profile created:\n$NEW_DESKTOP_FILE"

