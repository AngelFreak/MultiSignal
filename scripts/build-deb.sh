#!/bin/bash
# Builds an Ubuntu/Debian package: target/debian/multisignal_<version>_<arch>.deb
# Install it with:  sudo apt install ./target/debian/multisignal_*.deb
# It puts the binary in /usr/bin and "Signal Profiles" in the app launcher.
# GTK 4 and libadwaita come from the system (Ubuntu 24.04 or newer); the
# package depends on them, with versions worked out by dpkg-shlibdeps.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f $HOME/.cargo/env ]] && source "$HOME/.cargo/env"

cargo build --release --locked
version=$(cargo pkgid | sed 's/.*[#@]//')
arch=$(dpkg --print-architecture)
app_id=io.github.multisignal.MultiSignal
out=$PWD/target/debian
root=$out/root
rm -rf "$out"
mkdir -p "$root/DEBIAN"

install -Dm755 target/release/multisignal "$root/usr/bin/multisignal"
install -Dm644 "data/$app_id.desktop" "$root/usr/share/applications/$app_id.desktop"

# dpkg-shlibdeps wants a debian/control to exist; give it a throwaway one.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/debian"
printf 'Source: multisignal\n\nPackage: multisignal\nArchitecture: any\n' >"$work/debian/control"
depends=$(cd "$work" && dpkg-shlibdeps -O -e"$root/usr/bin/multisignal" |
    sed -n 's/^shlibs:Depends=//p')
[[ -n $depends ]] || { echo "could not work out the dependencies" >&2; exit 1; }

cat >"$root/DEBIAN/control" <<CONTROL
Package: multisignal
Version: $version
Architecture: $arch
Maintainer: $(git config user.name) <$(git config user.email)>
Installed-Size: $(du -sk --exclude=DEBIAN "$root" | cut -f1)
Depends: $depends
Recommends: snapd
Section: net
Priority: optional
Description: Run several Signal Desktop accounts side by side
 Signal Profiles creates, opens and removes separate Signal Desktop
 profiles, each with its own messages and app menu entry. It works with
 the Signal Desktop snap and installs it on request.
CONTROL

find "$root" -type d -exec chmod 755 {} +
deb=$out/multisignal_${version}_${arch}.deb
dpkg-deb --build --root-owner-group "$root" "$deb" >/dev/null
rm -rf "$root"
echo "$deb"
