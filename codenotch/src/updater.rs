//! Check this build's published releases (Windows or Ubuntu), then use a signed Tauri update when one exists.
//! Until signing is configured, Settings links to the official installer instead.

use semver::Version;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

#[cfg(windows)]
const REPO: &str = "vinzdg/codenotch";
#[cfg(windows)]
const INSTALLER_NAME: &str = "Codenotch-Setup.exe";
#[cfg(windows)]
const AGENT: &str = "Codenotch-Windows";
// The Linux build is released from its own repository, as a package rather than an installer
#[cfg(not(windows))]
const REPO: &str = "ikhode-technology/codenotch-ubuntu";
#[cfg(not(windows))]
const INSTALLER_NAME: &str = "Codenotch_amd64.deb";
#[cfg(not(windows))]
const AGENT: &str = "codenotch-ubuntu";
const UNSET_PUBKEY: &str = "REPLACE_WITH_TAURI_PUBLIC_KEY";
const CHECK_ERROR: &str = "Could not check for updates";
const INSTALL_ERROR: &str = "Could not install the update";
#[cfg(not(windows))]
const INSTALL_CANCELLED: &str = "Update cancelled";

#[derive(Clone, Serialize, Default)]
pub struct UpdateState {
    pub available: Option<String>,
    pub checking: bool,
    pub installing: bool,
    /// An unchecked build must not claim to be up to date.
    pub checked: bool,
    /// Tauri found a signed feed for this exact release.
    pub can_install: bool,
    pub message: Option<String>,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
}

static STATE: Mutex<Option<UpdateState>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<UpdateState>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn set(app: &AppHandle, next: UpdateState) {
    *state() = Some(next.clone());
    let _ = app.emit("update_state", &next);
}

#[tauri::command]
pub fn get_update_state() -> UpdateState {
    state().clone().unwrap_or_default()
}

fn configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.is_empty() && k != UNSET_PUBKEY)
}

/// A Mac-only release cannot be advertised as a Windows update.
fn newer_windows_release(body: &str, current: &Version) -> Result<Option<Version>, String> {
    let release: GithubRelease = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let tag = release
        .tag_name
        .strip_prefix('v')
        .ok_or("release tag has no v prefix")?;
    let version = Version::parse(tag).map_err(|e| e.to_string())?;
    if !release
        .assets
        .iter()
        .any(|asset| asset.name == INSTALLER_NAME)
    {
        return Err(format!(
            "release {} has no Windows installer",
            release.tag_name
        ));
    }
    Ok((version > *current).then_some(version))
}

fn latest_windows_release(current: &Version) -> Result<Option<Version>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(12))
        .build();
    let body = agent
        .get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", AGENT)
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    newer_windows_release(&body, current)
}

fn signed_offer_matches(release: &Version, signed: &str) -> bool {
    Version::parse(signed).is_ok_and(|version| version == *release)
}

fn newer_signed_release(signed: &str, current: &Version) -> Option<Version> {
    Version::parse(signed)
        .ok()
        .filter(|version| version > current)
}

fn signed_feed_offer(app: &AppHandle) -> Option<String> {
    match tauri::async_runtime::block_on(async { app.updater()?.check().await }) {
        Ok(Some(update)) => Some(update.version),
        Ok(None) => None,
        Err(e) => {
            crate::applog(&format!("updater: signed feed unavailable ({e})"));
            None
        }
    }
}

/// Returns immediately; release and signed-feed checks happen off the UI thread.
#[tauri::command]
pub fn check_for_update(app: AppHandle) {
    if state().as_ref().is_some_and(|s| s.checking || s.installing) {
        return;
    }
    set(
        &app,
        UpdateState {
            checking: true,
            ..Default::default()
        },
    );
    std::thread::spawn(move || {
        let current = match Version::parse(env!("CARGO_PKG_VERSION")) {
            Ok(version) => version,
            Err(e) => {
                crate::applog(&format!("updater: invalid installed version ({e})"));
                set(
                    &app,
                    UpdateState {
                        message: Some(CHECK_ERROR.into()),
                        ..Default::default()
                    },
                );
                return;
            }
        };
        match latest_windows_release(&current) {
            Ok(Some(version)) => {
                // Linux installs the release's own package (see `linux`); Windows needs the signed feed
                let can_install = cfg!(not(windows))
                    || configured(&app)
                    && signed_feed_offer(&app)
                        .is_some_and(|signed| signed_offer_matches(&version, &signed));
                crate::applog(&format!(
                    "updater: {version} available (signed={can_install})"
                ));
                set(
                    &app,
                    UpdateState {
                        available: Some(version.to_string()),
                        checked: true,
                        can_install,
                        ..Default::default()
                    },
                );
            }
            Ok(None) => {
                crate::applog("updater: this is the newest release");
                set(
                    &app,
                    UpdateState {
                        checked: true,
                        ..Default::default()
                    },
                );
            }
            Err(e) => {
                crate::applog(&format!("updater: release check failed ({e})"));
                // A working signed feed can still update a build when GitHub's
                // release API is unavailable or rate limited.
                let signed = configured(&app)
                    .then(|| signed_feed_offer(&app))
                    .flatten()
                    .and_then(|offer| newer_signed_release(&offer, &current));
                if let Some(version) = signed {
                    set(
                        &app,
                        UpdateState {
                            available: Some(version.to_string()),
                            checked: true,
                            can_install: true,
                            ..Default::default()
                        },
                    );
                } else {
                    set(
                        &app,
                        UpdateState {
                            message: Some(CHECK_ERROR.into()),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    });
}

/// Open only the GitHub asset named by a successful release check. The webview
/// supplies neither the version nor a URL.
#[tauri::command]
pub fn open_update_installer() -> Result<(), String> {
    let version = {
        let current = state();
        let offer = current.as_ref().ok_or("No update has been checked")?;
        if offer.checking || offer.installing || offer.can_install {
            return Err("No manual installer is available".into());
        }
        offer
            .available
            .clone()
            .ok_or("No Windows update is available")?
    };
    let version = Version::parse(&version).map_err(|e| e.to_string())?;
    let url = format!(
        "https://github.com/{REPO}/releases/download/v{version}/{INSTALLER_NAME}"
    );
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(not(windows))]
    command.arg(&url);
    #[cfg(windows)]
    let mut command = std::process::Command::new("cmd");
    #[cfg(windows)]
    command.args(["/C", "start", "", &url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Install only an offer previously found in a signed Tauri feed.
#[tauri::command]
pub fn install_update(app: AppHandle) {
    let offered = {
        let current = state();
        let Some(offer) = current.as_ref() else {
            return;
        };
        if offer.checking || offer.installing || !offer.can_install {
            return;
        }
        let Some(version) = offer.available.clone() else {
            return;
        };
        version
    };
    #[cfg(windows)]
    if !configured(&app) {
        return;
    }
    set(
        &app,
        UpdateState {
            available: Some(offered.clone()),
            installing: true,
            checked: true,
            can_install: true,
            ..Default::default()
        },
    );
    #[cfg(not(windows))]
    {
        std::thread::spawn(move || {
            let result = Version::parse(&offered).map_err(|_| INSTALL_ERROR).and_then(|v| linux::install(&v));
            match result {
                Ok(()) => {
                    crate::applog(&format!("updater: {offered} installed, restarting"));
                    linux::relaunch();
                    app.exit(0);
                }
                Err(message) => {
                    crate::applog(&format!("updater: {offered} not installed ({message})"));
                    set(
                        &app,
                        UpdateState {
                            available: Some(offered),
                            checked: true,
                            can_install: true,
                            message: Some(message.into()),
                            ..Default::default()
                        },
                    );
                }
            }
        });
        return;
    }
    #[cfg(windows)]
    std::thread::spawn(move || {
        let outcome = tauri::async_runtime::block_on(async {
            let Some(update) = app.updater()?.check().await? else {
                return Ok::<bool, tauri_plugin_updater::Error>(false);
            };
            if update.version != offered {
                return Ok(false);
            }
            // Tauri verifies the archive against the configured public key.
            update.download_and_install(|_, _| {}, || {}).await?;
            Ok(true)
        });
        match outcome {
            Ok(true) => crate::applog("updater: signed update installed, restarting"),
            Ok(false) => {
                crate::applog("updater: signed offer changed before installation");
                set(
                    &app,
                    UpdateState {
                        available: Some(offered),
                        checked: true,
                        message: Some(INSTALL_ERROR.into()),
                        ..Default::default()
                    },
                );
            }
            Err(e) => {
                crate::applog(&format!("updater: signed install failed ({e})"));
                set(
                    &app,
                    UpdateState {
                        available: Some(offered),
                        checked: true,
                        can_install: true,
                        message: Some(INSTALL_ERROR.into()),
                        ..Default::default()
                    },
                );
            }
        }
    });
}

/// The SHA-256 a release's SHA256SUMS gives for one of its files
#[cfg_attr(windows, allow(dead_code))]
fn checksum_for<'a>(sums: &'a str, file: &str) -> Option<&'a str> {
    sums.lines().find_map(|l| {
        let (hash, name) = l.split_once(char::is_whitespace)?;
        (name.trim_start_matches([' ', '*']) == file && hash.len() == 64).then_some(hash)
    })
}

/// One click from "available" to running it: the release's package, checked against its
/// SHA256SUMS, installed through the desktop's password prompt. An AppImage replaces itself and
/// needs no password.
#[cfg(not(windows))]
mod linux {
    use super::{checksum_for, Version, AGENT, INSTALLER_NAME, INSTALL_CANCELLED, INSTALL_ERROR, REPO};
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    const APPIMAGE_NAME: &str = "Codenotch_amd64.AppImage";
    /// Larger than any release asset; a response past it is not one
    const MAX_BYTES: u64 = 256 << 20;

    fn get(url: &str) -> Result<Vec<u8>, &'static str> {
        let response = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(600))
            .build()
            .get(url)
            .set("User-Agent", AGENT)
            .call()
            .map_err(|e| {
                crate::applog(&format!("updater: download failed {url}: {e}"));
                INSTALL_ERROR
            })?;
        let mut body = Vec::new();
        response.into_reader().take(MAX_BYTES).read_to_end(&mut body).map_err(|_| INSTALL_ERROR)?;
        Ok(body)
    }

    pub fn install(version: &Version) -> Result<(), &'static str> {
        let base = format!("https://github.com/{REPO}/releases/download/v{version}");
        let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
        let asset = if appimage.is_some() { APPIMAGE_NAME } else { INSTALLER_NAME };
        let sums = String::from_utf8(get(&format!("{base}/SHA256SUMS"))?).map_err(|_| INSTALL_ERROR)?;
        let want = checksum_for(&sums, asset).ok_or(INSTALL_ERROR)?.to_ascii_lowercase();
        let bytes = get(&format!("{base}/{asset}"))?;
        let got: String = ring::digest::digest(&ring::digest::SHA256, &bytes)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if got != want {
            crate::applog(&format!("updater: {asset} checksum mismatch"));
            return Err(INSTALL_ERROR);
        }
        if let Some(path) = appimage {
            // Written beside it and renamed over it: the running image stays intact until then
            let next = path.with_extension("new");
            std::fs::write(&next, &bytes).map_err(|_| INSTALL_ERROR)?;
            let _ = std::fs::set_permissions(&next, std::fs::Permissions::from_mode(0o755));
            return std::fs::rename(&next, &path).map_err(|_| INSTALL_ERROR);
        }
        // apt reads local packages as its own _apt user, which cannot see into a private dir
        let dir = std::env::temp_dir().join(format!("codenotch-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|_| INSTALL_ERROR)?;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
        let deb = dir.join(INSTALLER_NAME);
        let written = std::fs::write(&deb, &bytes);
        let _ = std::fs::set_permissions(&deb, std::fs::Permissions::from_mode(0o644));
        let status = written.ok().and_then(|_| {
            Command::new("pkexec").args(["apt-get", "install", "-y"]).arg(&deb).stdin(Stdio::null()).status().ok()
        });
        let _ = std::fs::remove_dir_all(&dir);
        match status.and_then(|s| s.code()) {
            Some(0) => Ok(()),
            // pkexec: the password prompt was dismissed, or the user may not install packages
            Some(126) | Some(127) => Err(INSTALL_CANCELLED),
            _ => Err(INSTALL_ERROR),
        }
    }

    /// Starts the new version once this one has gone: a second copy would otherwise find this
    /// one still running and hand itself over to it.
    pub fn relaunch() {
        let exe = std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or_else(|| "/usr/bin/codenotch".into());
        let _ = Command::new("sh")
            .args(["-c", "sleep 1; exec \"$0\""])
            .arg(exe)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

pub fn check_on_launch(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        check_for_update(app);
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn checksum_lines() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let sums = format!("{a}  Codenotch_amd64.deb\n{b} *Codenotch_amd64.AppImage\n");
        assert_eq!(super::checksum_for(&sums, "Codenotch_amd64.deb"), Some(a.as_str()));
        assert_eq!(super::checksum_for(&sums, "Codenotch_amd64.AppImage"), Some(b.as_str()));
        assert_eq!(super::checksum_for(&sums, "Codenotch_amd64"), None);
        assert_eq!(super::checksum_for("short  Codenotch_amd64.deb", "Codenotch_amd64.deb"), None);
    }

    use super::{newer_signed_release, newer_windows_release, signed_offer_matches};
    use semver::Version;

    fn release(tag: &str, windows: bool) -> String {
        let asset = if windows {
            format!(r#"[{{"name":"{}"}}]"#, super::INSTALLER_NAME)
        } else {
            "[]".to_string()
        };
        format!(r#"{{"tag_name":"{tag}","assets":{asset}}}"#)
    }

    #[test]
    fn compares_only_windows_releases() {
        let current = Version::parse("1.18.0").unwrap();
        assert_eq!(
            newer_windows_release(&release("v1.19.0", true), &current).unwrap(),
            Some(Version::parse("1.19.0").unwrap())
        );
        assert_eq!(
            newer_windows_release(&release("v1.18.0", true), &current).unwrap(),
            None
        );
        assert_eq!(
            newer_windows_release(&release("v1.17.0", true), &current).unwrap(),
            None
        );
        assert!(newer_windows_release(&release("v1.19.0", false), &current).is_err());
        assert!(newer_windows_release("not json", &current).is_err());
        assert!(newer_windows_release(&release("latest", true), &current).is_err());
    }

    #[test]
    fn signed_offer_must_match_the_release() {
        let release = Version::parse("1.19.0").unwrap();
        assert!(signed_offer_matches(&release, "1.19.0"));
        assert!(!signed_offer_matches(&release, "1.18.0"));
        assert!(!signed_offer_matches(&release, "bad version"));
        assert_eq!(
            newer_signed_release("1.19.0", &Version::parse("1.18.0").unwrap()),
            Some(release)
        );
        assert_eq!(
            newer_signed_release("1.18.0", &Version::parse("1.18.0").unwrap()),
            None
        );
    }
}
