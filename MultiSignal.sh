#!/bin/bash

set -e

# Constants
APPLICATIONS_DIR="$HOME/.local/share/applications"
SIGNAL_BASE_DIR="$HOME/Signal"
BACKUP_DIR="$HOME/Signal/backups"
CONFIG_DIR="$HOME/.config/multisignal"
CONFIG_FILE="$CONFIG_DIR/config"

# Error handling function
error_exit() {
    zenity --error --text="$1" 2>/dev/null || echo "ERROR: $1" >&2
    exit 1
}

# Info message function
info_message() {
    zenity --info --text="$1" 2>/dev/null || echo "INFO: $1"
}

# Warning message function
warning_message() {
    zenity --warning --text="$1" 2>/dev/null || echo "WARNING: $1"
}

# Check if zenity is available
if ! command -v zenity &>/dev/null; then
    echo "zenity is required but not installed. Install it with: sudo apt install zenity"
    exit 1
fi

# Check if pkexec is available
if ! command -v pkexec &>/dev/null; then
    error_exit "pkexec is required but not installed.\nInstall it with:\nsudo apt install policykit-1"
fi

# Create necessary directories
mkdir -p "$APPLICATIONS_DIR" "$SIGNAL_BASE_DIR" "$BACKUP_DIR" "$CONFIG_DIR"

# Sanitize profile name
sanitize_profile_name() {
    local name="$1"
    # Remove leading/trailing whitespace
    name=$(echo "$name" | xargs)
    # Replace spaces with underscores
    name="${name// /_}"
    # Remove all characters except alphanumeric, dash, and underscore
    name=$(echo "$name" | tr -cd '[:alnum:]_-')
    echo "$name"
}

# Validate profile name
validate_profile_name() {
    local name="$1"

    if [[ -z "$name" ]]; then
        error_exit "Profile name cannot be empty."
    fi

    if [[ ${#name} -lt 2 ]]; then
        error_exit "Profile name must be at least 2 characters long."
    fi

    if [[ ${#name} -gt 50 ]]; then
        error_exit "Profile name must be less than 50 characters."
    fi
}

# Check if profile exists
profile_exists() {
    local profile_name="$1"
    local desktop_file="$APPLICATIONS_DIR/Signal-$profile_name.desktop"
    [[ -f "$desktop_file" ]]
}

# Check if profile is running
is_profile_running() {
    local profile_name="$1"
    local profile_dir="$SIGNAL_BASE_DIR/$profile_name"
    pgrep -f "user-data-dir=$profile_dir" > /dev/null 2>&1
}

# Get running status
get_profile_status() {
    local profile_name="$1"
    if is_profile_running "$profile_name"; then
        echo "🟢 Running"
    else
        echo "⚫ Stopped"
    fi
}

# Install Signal Desktop if needed
install_signal_if_needed() {
    NEED_PRIVILEGE=false

    # Check if signal-desktop is installed
    if ! snap list 2>/dev/null | grep -q "^signal-desktop"; then
        NEED_PRIVILEGE=true
    fi

    # If we need to escalate privileges, ask the user
    if $NEED_PRIVILEGE; then
        zenity --question --title="Administrator Access Required" \
            --text="Signal Desktop is not installed.\n\nClick OK to install it now and enter your password when prompted." \
            --width=400

        if [[ $? -ne 0 ]]; then
            info_message "Installation cancelled."
            exit 0
        fi

        (
            echo "10"
            echo "# Checking snap..."
            sleep 0.5

            echo "30"
            echo "# Installing Signal Desktop..."
            sleep 0.5

            pkexec bash -c '
                set -e
                if ! snap list 2>/dev/null | grep -q "^signal-desktop"; then
                    snap install signal-desktop
                fi
            '

            echo "100"
            echo "# Installation complete."
            sleep 0.5
        ) | zenity --progress --title="Installing Requirements" \
                   --text="Preparing to install..." \
                   --percentage=0 --auto-close --width=400

        if [[ $? -ne 0 ]]; then
            error_exit "Installation failed. Please install Signal Desktop manually."
        fi
    fi
}

# Create new profile
create_profile() {
    local profile_name
    profile_name=$(zenity --entry --title="Create New Signal Profile" \
        --text="Enter a name for the new Signal profile:\n(Only letters, numbers, dash, and underscore allowed)")

    if [[ -z "$profile_name" ]]; then
        return
    fi

    # Sanitize the profile name
    local original_name="$profile_name"
    profile_name=$(sanitize_profile_name "$profile_name")

    # Notify user if name was changed
    if [[ "$original_name" != "$profile_name" ]]; then
        zenity --question --title="Profile Name Modified" \
            --text="Profile name was sanitized:\nOriginal: $original_name\nSanitized: $profile_name\n\nContinue with sanitized name?" \
            --width=400

        if [[ $? -ne 0 ]]; then
            return
        fi
    fi

    # Validate profile name
    validate_profile_name "$profile_name"

    # Check if profile already exists
    if profile_exists "$profile_name"; then
        error_exit "Profile '$profile_name' already exists.\nPlease choose a different name."
    fi

    local profile_dir="$SIGNAL_BASE_DIR/$profile_name"
    local new_desktop_file="$APPLICATIONS_DIR/Signal-$profile_name.desktop"

    mkdir -p "$profile_dir"

    cat > "$new_desktop_file" <<EOL
[Desktop Entry]
X-SnapInstanceName=signal-desktop
Name=Signal ($profile_name)
X-SnapAppName=signal-desktop
Exec=sh -c 'env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop /snap/bin/signal-desktop --user-data-dir=$HOME/Signal/$profile_name %U'
Terminal=false
Type=Application
Icon=/snap/signal-desktop/current/meta/gui/signal-desktop.png
StartupWMClass=Signal
Comment=Private messaging from your desktop ($profile_name)
MimeType=x-scheme-handler/sgnl;x-scheme-handler/signalcaptcha;
Categories=Network;InstantMessaging;Chat;
EOL

    chmod +x "$new_desktop_file"

    info_message "✅ Signal profile created successfully!\n\nProfile: $profile_name\nLauncher: $new_desktop_file\n\nYou can now launch it from your application menu."
}

# List all profiles
list_profiles() {
    mapfile -t PROFILE_FILES < <(find "$APPLICATIONS_DIR" -maxdepth 1 -name 'Signal-*.desktop' -printf '%f\n' 2>/dev/null | sort)

    if [[ ${#PROFILE_FILES[@]} -eq 0 ]]; then
        info_message "No Signal profiles found.\n\nCreate a new profile to get started."
        return
    fi

    local profile_list=""
    for FILE in "${PROFILE_FILES[@]}"; do
        local profile_name="${FILE#Signal-}"
        profile_name="${profile_name%.desktop}"
        local status=$(get_profile_status "$profile_name")
        local profile_dir="$SIGNAL_BASE_DIR/$profile_name"

        # Get profile size
        local size="Unknown"
        if [[ -d "$profile_dir" ]]; then
            size=$(du -sh "$profile_dir" 2>/dev/null | cut -f1)
        fi

        profile_list+="$profile_name\n$status\n$size\n"
    done

    zenity --list --title="Signal Profiles" \
        --text="Installed Signal Profiles (${#PROFILE_FILES[@]} total):" \
        --column="Profile Name" --column="Status" --column="Size" \
        --width=600 --height=400 \
        $(echo -e "$profile_list")
}

# Launch profile
launch_profile() {
    mapfile -t PROFILE_FILES < <(find "$APPLICATIONS_DIR" -maxdepth 1 -name 'Signal-*.desktop' -printf '%f\n' 2>/dev/null | sort)

    if [[ ${#PROFILE_FILES[@]} -eq 0 ]]; then
        info_message "No Signal profiles found.\n\nCreate a new profile first."
        return
    fi

    # Build zenity list input
    local zenity_input=()
    for FILE in "${PROFILE_FILES[@]}"; do
        local profile_name="${FILE#Signal-}"
        profile_name="${profile_name%.desktop}"
        local status=$(get_profile_status "$profile_name")
        zenity_input+=("$profile_name" "$status")
    done

    local selected
    selected=$(zenity --list --title="Launch Signal Profile" \
        --text="Select a profile to launch:" \
        --column="Profile Name" --column="Status" \
        "${zenity_input[@]}" \
        --width=500 --height=400)

    if [[ -z "$selected" ]]; then
        return
    fi

    local profile_dir="$SIGNAL_BASE_DIR/$selected"

    # Check if already running
    if is_profile_running "$selected"; then
        warning_message "Profile '$selected' is already running."
        return
    fi

    # Launch the profile
    nohup /snap/bin/signal-desktop --user-data-dir="$profile_dir" > /dev/null 2>&1 &

    info_message "🚀 Launching Signal profile: $selected"
}

# Rename profile
rename_profile() {
    mapfile -t PROFILE_FILES < <(find "$APPLICATIONS_DIR" -maxdepth 1 -name 'Signal-*.desktop' -printf '%f\n' 2>/dev/null | sort)

    if [[ ${#PROFILE_FILES[@]} -eq 0 ]]; then
        info_message "No Signal profiles found to rename."
        return
    fi

    # Build zenity list input
    local zenity_input=()
    for FILE in "${PROFILE_FILES[@]}"; do
        local profile_name="${FILE#Signal-}"
        profile_name="${profile_name%.desktop}"
        zenity_input+=("$profile_name")
    done

    local old_name
    old_name=$(zenity --list --title="Rename Signal Profile" \
        --text="Select a profile to rename:" \
        --column="Profile Name" \
        "${zenity_input[@]}" \
        --width=400 --height=400)

    if [[ -z "$old_name" ]]; then
        return
    fi

    # Check if profile is running
    if is_profile_running "$old_name"; then
        error_exit "Cannot rename profile '$old_name' while it's running.\n\nPlease close the profile first."
    fi

    local new_name
    new_name=$(zenity --entry --title="Rename Profile" \
        --text="Enter new name for profile '$old_name':" \
        --entry-text="$old_name")

    if [[ -z "$new_name" ]]; then
        return
    fi

    # Sanitize the new name
    local original_new_name="$new_name"
    new_name=$(sanitize_profile_name "$new_name")

    # Notify user if name was changed
    if [[ "$original_new_name" != "$new_name" ]]; then
        zenity --question --title="Profile Name Modified" \
            --text="Profile name was sanitized:\nOriginal: $original_new_name\nSanitized: $new_name\n\nContinue with sanitized name?" \
            --width=400

        if [[ $? -ne 0 ]]; then
            return
        fi
    fi

    # Validate new profile name
    validate_profile_name "$new_name"

    # Check if new name already exists
    if profile_exists "$new_name"; then
        error_exit "Profile '$new_name' already exists.\nPlease choose a different name."
    fi

    # Rename profile directory
    local old_dir="$SIGNAL_BASE_DIR/$old_name"
    local new_dir="$SIGNAL_BASE_DIR/$new_name"

    if [[ -d "$old_dir" ]]; then
        mv "$old_dir" "$new_dir"
    fi

    # Update desktop file
    local old_desktop="$APPLICATIONS_DIR/Signal-$old_name.desktop"
    local new_desktop="$APPLICATIONS_DIR/Signal-$new_name.desktop"

    cat > "$new_desktop" <<EOL
[Desktop Entry]
X-SnapInstanceName=signal-desktop
Name=Signal ($new_name)
X-SnapAppName=signal-desktop
Exec=sh -c 'env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop /snap/bin/signal-desktop --user-data-dir=$HOME/Signal/$new_name %U'
Terminal=false
Type=Application
Icon=/snap/signal-desktop/current/meta/gui/signal-desktop.png
StartupWMClass=Signal
Comment=Private messaging from your desktop ($new_name)
MimeType=x-scheme-handler/sgnl;x-scheme-handler/signalcaptcha;
Categories=Network;InstantMessaging;Chat;
EOL

    chmod +x "$new_desktop"
    rm -f "$old_desktop"

    info_message "✅ Profile renamed successfully!\n\nOld name: $old_name\nNew name: $new_name"
}

# Backup profile
backup_profile() {
    mapfile -t PROFILE_FILES < <(find "$APPLICATIONS_DIR" -maxdepth 1 -name 'Signal-*.desktop' -printf '%f\n' 2>/dev/null | sort)

    if [[ ${#PROFILE_FILES[@]} -eq 0 ]]; then
        info_message "No Signal profiles found to backup."
        return
    fi

    # Build zenity list input
    local zenity_input=()
    for FILE in "${PROFILE_FILES[@]}"; do
        local profile_name="${FILE#Signal-}"
        profile_name="${profile_name%.desktop}"
        local status=$(get_profile_status "$profile_name")
        zenity_input+=("$profile_name" "$status")
    done

    local selected
    selected=$(zenity --list --title="Backup Signal Profile" \
        --text="Select a profile to backup:" \
        --column="Profile Name" --column="Status" \
        "${zenity_input[@]}" \
        --width=500 --height=400)

    if [[ -z "$selected" ]]; then
        return
    fi

    local profile_dir="$SIGNAL_BASE_DIR/$selected"

    if [[ ! -d "$profile_dir" ]]; then
        error_exit "Profile directory not found: $profile_dir"
    fi

    # Create backup with timestamp
    local timestamp=$(date +"%Y%m%d_%H%M%S")
    local backup_file="$BACKUP_DIR/${selected}_${timestamp}.tar.gz"

    (
        echo "10"
        echo "# Preparing backup..."
        sleep 0.5

        echo "30"
        echo "# Compressing profile data..."

        tar -czf "$backup_file" -C "$SIGNAL_BASE_DIR" "$selected" 2>&1

        echo "90"
        echo "# Finalizing..."
        sleep 0.5

        echo "100"
        echo "# Backup complete!"
    ) | zenity --progress --title="Backing Up Profile" \
               --text="Creating backup..." \
               --percentage=0 --auto-close --width=400

    info_message "✅ Backup created successfully!\n\nProfile: $selected\nBackup file: $backup_file\n\nSize: $(du -h "$backup_file" | cut -f1)"
}

# Restore profile
restore_profile() {
    mapfile -t BACKUP_FILES < <(find "$BACKUP_DIR" -maxdepth 1 -name '*.tar.gz' -printf '%f\n' 2>/dev/null | sort -r)

    if [[ ${#BACKUP_FILES[@]} -eq 0 ]]; then
        info_message "No backup files found in:\n$BACKUP_DIR"
        return
    fi

    # Build zenity list input
    local zenity_input=()
    for FILE in "${BACKUP_FILES[@]}"; do
        local size=$(du -h "$BACKUP_DIR/$FILE" | cut -f1)
        zenity_input+=("$FILE" "$size")
    done

    local selected
    selected=$(zenity --list --title="Restore Signal Profile" \
        --text="Select a backup to restore:" \
        --column="Backup File" --column="Size" \
        "${zenity_input[@]}" \
        --width=600 --height=400)

    if [[ -z "$selected" ]]; then
        return
    fi

    local backup_file="$BACKUP_DIR/$selected"

    # Extract profile name from backup filename
    local profile_name="${selected%%_*}"

    # Check if profile already exists
    if profile_exists "$profile_name"; then
        zenity --question --title="Profile Already Exists" \
            --text="Profile '$profile_name' already exists.\n\nDo you want to overwrite it?" \
            --width=400

        if [[ $? -ne 0 ]]; then
            return
        fi

        # Check if running
        if is_profile_running "$profile_name"; then
            error_exit "Cannot restore profile '$profile_name' while it's running.\n\nPlease close the profile first."
        fi
    fi

    (
        echo "10"
        echo "# Preparing restore..."
        sleep 0.5

        echo "30"
        echo "# Extracting backup..."

        tar -xzf "$backup_file" -C "$SIGNAL_BASE_DIR" 2>&1

        echo "70"
        echo "# Creating desktop launcher..."

        # Create desktop file
        local new_desktop_file="$APPLICATIONS_DIR/Signal-$profile_name.desktop"
        cat > "$new_desktop_file" <<EOL
[Desktop Entry]
X-SnapInstanceName=signal-desktop
Name=Signal ($profile_name)
X-SnapAppName=signal-desktop
Exec=sh -c 'env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop /snap/bin/signal-desktop --user-data-dir=$HOME/Signal/$profile_name %U'
Terminal=false
Type=Application
Icon=/snap/signal-desktop/current/meta/gui/signal-desktop.png
StartupWMClass=Signal
Comment=Private messaging from your desktop ($profile_name)
MimeType=x-scheme-handler/sgnl;x-scheme-handler/signalcaptcha;
Categories=Network;InstantMessaging;Chat;
EOL

        chmod +x "$new_desktop_file"

        echo "100"
        echo "# Restore complete!"
    ) | zenity --progress --title="Restoring Profile" \
               --text="Restoring backup..." \
               --percentage=0 --auto-close --width=400

    info_message "✅ Profile restored successfully!\n\nProfile: $profile_name\nFrom backup: $selected"
}

# Delete profile
delete_profile() {
    mapfile -t PROFILE_FILES < <(find "$APPLICATIONS_DIR" -maxdepth 1 -name 'Signal-*.desktop' -printf '%f\n' 2>/dev/null | sort)

    if [[ ${#PROFILE_FILES[@]} -eq 0 ]]; then
        info_message "No Signal profiles found to delete."
        return
    fi

    # Build zenity checklist input
    local zenity_input=()
    for FILE in "${PROFILE_FILES[@]}"; do
        local profile_name="${FILE#Signal-}"
        profile_name="${profile_name%.desktop}"
        local status=$(get_profile_status "$profile_name")
        zenity_input+=("FALSE" "$profile_name" "$status")
    done

    local selected
    selected=$(zenity --list --checklist \
        --title="Delete Signal Profiles" \
        --text="Select one or more profiles to delete:" \
        --column="Select" --column="Profile" --column="Status" \
        "${zenity_input[@]}" \
        --width=500 --height=400)

    if [[ -z "$selected" ]]; then
        return
    fi

    IFS="|" read -ra PROFILES_TO_DELETE <<< "$selected"

    # Check if any selected profile is running
    local running_profiles=()
    for profile in "${PROFILES_TO_DELETE[@]}"; do
        if is_profile_running "$profile"; then
            running_profiles+=("$profile")
        fi
    done

    if [[ ${#running_profiles[@]} -gt 0 ]]; then
        error_exit "Cannot delete running profiles:\n${running_profiles[*]}\n\nPlease close them first."
    fi

    # Ask if user wants to backup before deletion
    local backup_first=false
    zenity --question --title="Backup Before Deletion" \
        --text="Do you want to backup the selected profiles before deletion?\n\nThis is recommended for safety." \
        --width=400

    if [[ $? -eq 0 ]]; then
        backup_first=true
    fi

    # Perform backups if requested
    if $backup_first; then
        for profile in "${PROFILES_TO_DELETE[@]}"; do
            local profile_dir="$SIGNAL_BASE_DIR/$profile"

            if [[ -d "$profile_dir" ]]; then
                local timestamp=$(date +"%Y%m%d_%H%M%S")
                local backup_file="$BACKUP_DIR/${profile}_${timestamp}.tar.gz"

                tar -czf "$backup_file" -C "$SIGNAL_BASE_DIR" "$profile" 2>&1 | \
                    zenity --progress --title="Backing Up" \
                           --text="Backing up $profile..." \
                           --pulsate --auto-close --width=400 || true
            fi
        done
    fi

    # Confirm deletion
    local confirm_text="The following profiles will be PERMANENTLY deleted:\n\n"
    for profile in "${PROFILES_TO_DELETE[@]}"; do
        confirm_text+="  • $profile\n"
    done
    confirm_text+="\nThis will remove both launchers and data directories."

    if $backup_first; then
        confirm_text+="\n\n(Backups have been created in $BACKUP_DIR)"
    fi

    zenity --question --title="⚠️ Confirm Deletion" \
        --text="$confirm_text" \
        --width=500

    if [[ $? -ne 0 ]]; then
        info_message "Deletion cancelled."
        return
    fi

    # Perform deletion
    for profile in "${PROFILES_TO_DELETE[@]}"; do
        local desktop_file="$APPLICATIONS_DIR/Signal-$profile.desktop"
        local profile_dir="$SIGNAL_BASE_DIR/$profile"
        rm -f "$desktop_file"
        rm -rf "$profile_dir"
    done

    info_message "✅ Selected profiles have been deleted.\n\nDeleted profiles: ${#PROFILES_TO_DELETE[@]}"
}

# Main menu
main_menu() {
    while true; do
        local action
        action=$(zenity --list --radiolist \
            --title="MultiSignal - Signal Profile Manager" \
            --text="Choose an action:" \
            --column="Select" --column="Action" --column="Description" \
            TRUE "Create Profile" "Create a new Signal profile" \
            FALSE "Launch Profile" "Launch an existing profile" \
            FALSE "List Profiles" "View all profiles and their status" \
            FALSE "Rename Profile" "Rename an existing profile" \
            FALSE "Backup Profile" "Create a backup of a profile" \
            FALSE "Restore Profile" "Restore a profile from backup" \
            FALSE "Delete Profile" "Delete one or more profiles" \
            FALSE "Exit" "Close this application" \
            --width=600 --height=450)

        if [[ -z "$action" ]] || [[ "$action" == "Exit" ]]; then
            exit 0
        fi

        case "$action" in
            "Create Profile")
                create_profile
                ;;
            "Launch Profile")
                launch_profile
                ;;
            "List Profiles")
                list_profiles
                ;;
            "Rename Profile")
                rename_profile
                ;;
            "Backup Profile")
                backup_profile
                ;;
            "Restore Profile")
                restore_profile
                ;;
            "Delete Profile")
                delete_profile
                ;;
        esac
    done
}

# Main execution
install_signal_if_needed
main_menu
