# MultiSignal

A comprehensive shell script to make managing multiple Signal Desktop instances user-friendly and safe.

## Overview

MultiSignal is a Shell script designed to streamline and simplify the management of multiple Signal instances. This tool provides an easy and user-friendly experience for users who need to operate several Signal sessions simultaneously with robust safety features and backup capabilities.

## Features

### Core Features
- **Create Profiles** - Easy setup for multiple Signal accounts with automatic desktop launchers
- **Launch Profiles** - Directly launch any profile from the application
- **List Profiles** - View all profiles with status (running/stopped) and disk usage
- **Rename Profiles** - Safely rename existing profiles
- **Backup & Restore** - Full backup and restore functionality for profile data
- **Delete Profiles** - Remove profiles with optional automatic backup

### Safety & Validation
- **Input Sanitization** - Automatic profile name validation and sanitization
- **Conflict Detection** - Prevents creating duplicate profiles
- **Running Instance Detection** - Cannot delete or modify profiles that are currently running
- **Backup Before Deletion** - Optional automatic backup when deleting profiles
- **Error Handling** - Comprehensive error messages and validation

### User Experience
- **User-friendly GUI** - Intuitive Zenity-based interface
- **Status Indicators** - Visual indicators for running/stopped profiles
- **Disk Usage Tracking** - See how much space each profile uses
- **Progress Bars** - Visual feedback for long operations
- **Main Menu Loop** - Perform multiple operations without restarting

### Technical Features
- **100% Shell script** - No external dependencies except Signal and standard Unix tools
- **Automatic Signal Installation** - Installs Signal Desktop if not present
- **Safe Profile Isolation** - Each profile has its own data directory
- **Backup Management** - Timestamped backups stored in organized directory

## Requirements

- Bash or compatible shell
- Ubuntu (or compatible Linux distribution)
- zenity (for GUI dialogs)
- snap (for Signal Desktop installation)
- pkexec (for privilege escalation)

## Installation

### Quick Start

```sh
git clone https://github.com/AngelFreak/MultiSignal.git
cd MultiSignal
chmod +x MultiSignal.sh
./MultiSignal.sh
```

The script will automatically install Signal Desktop if it's not already installed.

### Manual Dependency Installation

If needed, install dependencies manually:

```sh
sudo apt install zenity policykit-1
sudo snap install signal-desktop
```

## Usage

### Main Menu

When you run the script, you'll see a main menu with the following options:

1. **Create Profile** - Create a new Signal profile
2. **Launch Profile** - Launch an existing profile
3. **List Profiles** - View all profiles and their status
4. **Rename Profile** - Rename an existing profile
5. **Backup Profile** - Create a backup of a profile
6. **Restore Profile** - Restore a profile from backup
7. **Delete Profile** - Delete one or more profiles
8. **Exit** - Close the application

### Creating a Profile

1. Select "Create Profile" from the main menu
2. Enter a name for your profile (letters, numbers, dash, and underscore only)
3. The script will sanitize the name if needed
4. A new desktop launcher will be created in your application menu

### Launching a Profile

1. Select "Launch Profile" from the main menu
2. Choose the profile you want to launch
3. The profile will open in a new Signal window
4. You can launch multiple profiles simultaneously

### Backing Up a Profile

1. Select "Backup Profile" from the main menu
2. Choose the profile to backup
3. A timestamped backup file will be created in `~/Signal/backups/`
4. Backups include all profile data and can be restored later

### Deleting a Profile

1. Select "Delete Profile" from the main menu
2. Select one or more profiles to delete
3. Optionally create a backup before deletion (recommended)
4. Confirm the deletion
5. The profile data and launcher will be permanently removed

**Note:** You cannot delete a profile that is currently running.

## File Structure

```
~/Signal/                          # Profile data directory
  ├── ProfileName1/                # Individual profile data
  ├── ProfileName2/
  └── backups/                     # Backup storage
      ├── ProfileName1_20231215_143022.tar.gz
      └── ProfileName2_20231215_143055.tar.gz

~/.local/share/applications/       # Desktop launchers
  ├── Signal-ProfileName1.desktop
  └── Signal-ProfileName2.desktop

~/.config/multisignal/             # Configuration directory
  └── config                       # Future configuration file
```

## Safety Features

### Profile Name Sanitization
- Automatically removes special characters
- Replaces spaces with underscores
- Ensures profile names are filesystem-safe
- Validates name length (2-50 characters)

### Running Instance Protection
- Detects if a profile is currently running
- Prevents deletion of active profiles
- Prevents renaming of active profiles
- Prevents restoration over active profiles

### Backup Protection
- Optional automatic backup before deletion
- Timestamped backups prevent overwrites
- Can restore from any previous backup
- Backups are compressed to save space

## Troubleshooting

### Signal Desktop won't install
- Ensure snap is installed: `sudo apt install snapd`
- Try manual installation: `sudo snap install signal-desktop`

### Profiles don't appear in application menu
- Run: `update-desktop-database ~/.local/share/applications/`
- Log out and log back in

### Profile won't launch
- Verify Signal Desktop is installed: `snap list signal-desktop`
- Check profile directory exists: `ls ~/Signal/`
- Try launching directly: `signal-desktop --user-data-dir=~/Signal/ProfileName`

### Can't delete a profile
- Ensure the profile is not running
- Check process: `ps aux | grep signal-desktop`
- Kill the process if needed: `pkill -f "user-data-dir=.*ProfileName"`

### Backup/Restore fails
- Ensure you have enough disk space
- Check backup directory permissions: `ls -la ~/Signal/backups/`
- Verify tar is installed: `which tar`

## Advanced Usage

### Command-line Profile Launch
You can launch a specific profile directly from the command line:

```sh
signal-desktop --user-data-dir=~/Signal/ProfileName
```

### Manual Backup
Create a manual backup:

```sh
tar -czf ~/Signal/backups/ProfileName_$(date +%Y%m%d_%H%M%S).tar.gz -C ~/Signal ProfileName
```

### Manual Restore
Restore a backup manually:

```sh
tar -xzf ~/Signal/backups/ProfileName_20231215_143022.tar.gz -C ~/Signal
```

## Known Limitations

- Currently only supports snap-based Signal Desktop installations
- Ubuntu/Debian-focused (uses apt and snap)
- Requires Zenity for GUI (no CLI-only mode yet)
- Profile icons cannot be customized (uses default Signal icon)
- No automatic profile synchronization between machines

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

### Feature Requests

Future enhancements could include:
- Flatpak and AppImage support
- Command-line interface mode
- Automatic backup scheduling
- Profile templates
- Custom profile icons
- Multi-distro support (Fedora, Arch, etc.)
- Profile import/export for migration between machines

## License

This project is open source. Please check the repository for license details.

## Changelog

### Version 2.0 (Current)
- Added profile listing with status and disk usage
- Added direct profile launching
- Added profile renaming
- Added backup and restore functionality
- Added input validation and sanitization
- Added running instance detection
- Added backup before deletion option
- Improved error handling
- Added main menu loop for multiple operations
- Enhanced user feedback with progress bars

### Version 1.0
- Basic profile creation
- Basic profile deletion
- Desktop launcher generation

## Author

Created by AngelFreak

## Support

For issues, questions, or feature requests, please visit:
https://github.com/AngelFreak/MultiSignal/issues
