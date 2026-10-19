#!/bin/sh
# Codenotch for Ubuntu — installs the latest release:
#
#   curl -fsSL https://raw.githubusercontent.com/ikhode-technology/codenotch-ubuntu/main/install.sh | sh
#
# Downloads the .deb from the newest GitHub release, installs it with apt (which pulls in
# WebKitGTK and the tray library), and starts the notch. Run it again to update.
set -eu

REPO=ikhode-technology/codenotch-ubuntu
URL="https://github.com/$REPO/releases/latest/download/Codenotch_amd64.deb"

say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf 'codenotch: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Linux ] || die "this installer is for Ubuntu/Linux."
command -v apt-get >/dev/null 2>&1 || die "apt was not found. On other distributions use the AppImage: https://github.com/$REPO/releases/latest"
command -v curl >/dev/null 2>&1 || die "curl is needed: sudo apt install curl"
[ "$(dpkg --print-architecture)" = amd64 ] || die "only amd64 (x86_64) builds are published."

# apt reads local packages as its own _apt user, which cannot see into a private temp dir
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
chmod 755 "$tmp"

say "Downloading Codenotch…"
curl -fL --progress-bar -o "$tmp/codenotch.deb" "$URL" || die "download failed: $URL"
chmod 644 "$tmp/codenotch.deb"

say "Installing (sudo will ask for your password)…"
if [ "$(id -u)" -eq 0 ]; then SUDO=; else SUDO=sudo; fi
$SUDO apt-get install -y "$tmp/codenotch.deb"

# Restart it when a desktop session is there to show it in
if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && [ "$(id -u)" -ne 0 ]; then
  pkill -x codenotch 2>/dev/null || true
  # A terminal started from a snap (VS Code, for one) exports library paths that break the app
  env -u LD_LIBRARY_PATH -u GTK_PATH -u GIO_MODULE_DIR -u GSETTINGS_SCHEMA_DIR \
      -u GDK_PIXBUF_MODULE_FILE -u GDK_PIXBUF_MODULEDIR \
      nohup /usr/bin/codenotch >/dev/null 2>&1 &
  say "Codenotch is running — look at the right edge of your screen."
else
  say "Installed. Start Codenotch from your app menu."
fi
echo "Uninstall any time with: sudo apt remove codenotch"
