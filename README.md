<div align="center">

![codenotch-ubuntu](docs/banner.jpg)

# codenotch-ubuntu

![Platform](https://img.shields.io/badge/platform-Ubuntu%2022.04%2B-E95420?logo=ubuntu&logoColor=white)
![Rust](https://img.shields.io/badge/rust-Tauri%202-orange)
![License](https://img.shields.io/badge/license-MIT-green)

**A small black notch pinned to the edge of your screen, showing how much of each AI coding
assistant's usage limit you have burned — and whether Claude is still working, done, or waiting
on you.**

Provided by **Meatika Dev Team**

</div>

## Quick install

Open a terminal and run:

```sh
curl -fsSL https://raw.githubusercontent.com/ikhode-technology/codenotch-ubuntu/main/install.sh | sh
```

That downloads the latest release, installs it with `apt` (asking for your password once) and
starts the notch on the right edge of your screen. Run the same command again to update.

<details>
<summary>Prefer to install by hand?</summary>

Download `Codenotch_amd64.deb` from the [latest release](../../releases/latest), then:

```sh
sudo apt install ./Codenotch_amd64.deb
```

Not on Ubuntu or Debian? The release also has `Codenotch_amd64.AppImage`:
`chmod +x Codenotch_amd64.AppImage && ./Codenotch_amd64.AppImage`.

</details>

Works on Ubuntu 22.04 and newer (and other Debian-based distributions), on x86-64.

## How to use it

1. **Start it** — the installer starts it for you. Later, open **Codenotch** from the app menu or
   run `codenotch` in a terminal. Turn on *Settings → Open Codenotch at login* to have it always there.
2. **Sign in to your tools as usual** — there is nothing to connect. Each assistant that is installed
   and signed in gets a ring by itself (for ChatGPT Plus/Pro: `codex login`, *Sign in with ChatGPT*).
3. **Hover** a ring to see its card: every limit window, how much is used and when it resets.
4. **Click** a ring to refresh it. **Right-click** for *Refresh now*, the provider's usage page and *Quit*.
5. **Drag** the six dots beside the settings button (or hold <kbd>Alt</kbd> on the notch) to move it
   along any edge of any screen.
6. **Settings** (the gear below the rings, or the tray icon): which rings to show, size, edge,
   *Show on hover*, theme, language, start at login, *Check for updates*, and **Install hooks** for
   live Claude session status. Clicking a finished Claude session jumps back to its terminal window.


## What it shows

Each assistant that is installed and signed in on your machine gets a ring. Codenotch borrows the
sign-in that tool already has — there is nothing to log into here, and it never writes those files.

| Ring | Where it reads from |
|---|---|
| **Claude Code** | The token in `~/.claude/.credentials.json`, against the same usage endpoint `/usage` asks. Current session and weekly limits. A thin arc spins while a session is working and pulses when one is waiting on you. |
| **Codex** | The Codex sign-in in `~/.codex/auth.json` — 5-hour and weekly windows — falling back to the newest session snapshot. |
| **Cursor** | The editor's own session in `~/.config/Cursor/User/globalStorage/state.vscdb`. Included and on-demand usage. |
| **GitHub Copilot** | The GitHub CLI session (`gh auth login`). Premium requests, chat and completions. |
| **Grok** | The Grok CLI session in `~/.grok/auth.json`. The weekly allowance. |
| **Antigravity** | The official `agy` CLI's `/usage` when it is installed; otherwise the local language server. |
| **OpenCode Go** | OpenCode's sign-in. 5-hour, weekly and monthly windows. |
| **GLM** | The Z.ai Coding Plan key from Claude Code's `settings.json`, ZCode or OpenCode. |

Tools that are not installed simply do not get a ring.

## Files and diagnostics

Settings, logs and saved readings live in `~/.config/codenotch`. Run `codenotch doctor` in a terminal
to see what it found: credentials, CLIs, sessions.

## Update and uninstall

```sh
# update: the install command again
curl -fsSL https://raw.githubusercontent.com/ikhode-technology/codenotch-ubuntu/main/install.sh | sh

# uninstall
sudo apt remove codenotch
```

Settings → General → *Check for updates* asks this repository's releases whether a newer version is out.

## Build from source

```sh
sudo apt install build-essential pkg-config libssl-dev libwebkit2gtk-4.1-dev \
                 libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev curl
curl https://sh.rustup.rs -sSf | sh          # if you have no Rust toolchain yet
cargo build --release -p codenotch -p codenotch-hook
./scripts/run-linux.sh                       # run it
./scripts/package-linux.sh                   # build the .deb and AppImage (needs Node for npx)
```

## Troubleshooting

| Problem | Fix |
|---|---|
| No tray icon on GNOME | Install the *AppIndicator and KStatusNotifierItem Support* extension. |
| Wayland session | The notch runs through XWayland automatically (Wayland lets no app place its own window on a screen edge). |
| `symbol lookup error … __libc_pthread_init` when started from VS Code's terminal | That terminal is a snap and leaks its libraries; start Codenotch from the app menu, or with `./scripts/run-linux.sh`. |
| A card or ring stays on screen after closing (NVIDIA) | Fixed in the app: Ubuntu's WebKitGTK turns GPU rendering off under the NVIDIA driver, so Codenotch opts back in with shared-memory frames. Set `WEBKIT_FORCE_DMABUF_RENDERER=0` to opt out. |
| Antigravity ring missing | Install the `agy` CLI, or `sudo apt install libsecret-tools` for the credential fallback. |

## How this port differs from the Windows one

It is the upstream [`windows/`](https://github.com/vinzdg/codenotch/tree/main/windows) Rust + Tauri 2
port, with what Linux was missing filled in: dragging the notch (X11 pointer), jumping back to a
session's terminal and seen-clears-it (`/proc` + EWMH), Antigravity through `agy` in a pseudo-terminal
and the Secret Service, Codex's native client behind npm's shim, the notch size setting under
WebKitGTK's zoom, smooth card animation, a `.deb`/AppImage, and updates from this repository's releases.

## Credits and license

codenotch-ubuntu is provided by **Meatika Dev Team**. Codenotch is designed by [Vinz](https://github.com/vinzdg/codenotch); the Windows port this builds on is
by [Im-Midi](https://github.com/Im-Midi/codenotch-windows). Provider marks are from
[`@lobehub/icons`](https://github.com/lobehub/lobe-icons) (MIT) and remain their owners' trademarks.

MIT — see [`LICENSE`](LICENSE).
