# MultiSignal

**Signal Profiles**: run several Signal Desktop accounts side by side on Ubuntu,
each with its own messages and its own entry in the app menu.

A native GTK 4 / libadwaita app with an Apple-style look (light and dark). It
replaces the original `MultiSignal.sh` Bash + zenity script, which is kept in
[`legacy/`](legacy/) and uses the same files on disk, so both work side by side.

## Features

- Create, open and move profiles to the Trash (restorable from Files).
- Live status: which profiles are running, and how much space each uses.
- Shows the Signal you open from the normal **Signal** menu entry as a
  built-in "(default)" profile.
- Repairs missing or outdated app menu entries; leaves hand-made ones alone.
- Installs the Signal Desktop snap if it's missing.
- Appearance: Automatic (follows GNOME), Light or Dark, from the **⋯** menu.

## Install

Needs Ubuntu 24.04 or newer and the Signal Desktop snap (the app offers to
install it).

1. Download `multisignal_<version>_amd64.deb` from
   [Releases](https://github.com/AngelFreak/MultiSignal/releases).
2. Install it:

   ```sh
   sudo apt install ./multisignal_*_amd64.deb
   ```

   apt may print a note that the download is "performed unsandboxed as root"
   when the file is in your home folder; that's harmless.
3. Open **Signal Profiles** from the app launcher.

Remove it with `sudo apt remove multisignal`. Your profiles are not touched.

## How profiles are stored

| What | Where |
|---|---|
| Profile data | `~/Signal/<name>/` |
| App menu entry | `~/.local/share/applications/Signal-<name>.desktop` |
| Default Signal (snap) | `~/snap/signal-desktop/current/.config/Signal` |
| App settings | `~/.config/multisignal/settings.ini` |

Profile names use letters, digits, `.`, `_` and `-`, and start with a letter or
digit. To give the default Signal your own name, add this to `settings.ini`:

```ini
[default]
name=Personal
```

## Build from source

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev dpkg-dev desktop-file-utils
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust toolchain

make install     # into ~/.local (make uninstall removes it)
make deb         # target/debian/multisignal_<version>_<arch>.deb
```

## Development

```sh
cargo test               # unit, store, system and UI tests (UI tests need a display)
scripts/ui-smoke.sh      # renders every screen to target/ui-shots/*.png
```

The core (`names`, `paths`, `launcher`, `procs`, `profiles`, `store`) has no
GTK dependency; `system` holds the real side effects and `ui` only renders
state. See [`docs/plans/`](docs/plans/) for the design and the approved mockup.

## Releasing

Set the version in `Cargo.toml`, commit, then tag it:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

The [Release workflow](.github/workflows/release.yml) runs the tests, builds
the `.deb` and attaches it to the GitHub release for that tag.
