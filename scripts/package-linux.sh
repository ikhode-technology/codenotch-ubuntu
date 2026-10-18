#!/bin/sh
# Builds the .deb and AppImage, then copies them into dist/ under the fixed names a release carries:
# install.sh and the in-app update check download releases/latest/download/Codenotch_amd64.deb.
# The hook gets its own target dir, so the bundler never copies it onto itself.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
cargo build --release --locked -p codenotch-hook --target-dir target/hook
(cd codenotch && npx --yes @tauri-apps/cli@2.11.4 build)
rm -rf dist && mkdir dist
cp target/release/bundle/deb/Codenotch_*_amd64.deb dist/Codenotch_amd64.deb
cp target/release/bundle/appimage/Codenotch_*_amd64.AppImage dist/Codenotch_amd64.AppImage
(cd dist && sha256sum Codenotch_amd64.deb Codenotch_amd64.AppImage > SHA256SUMS)
ls -l dist
