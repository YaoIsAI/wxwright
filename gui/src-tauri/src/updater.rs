//! In-app updates through the Tauri updater plugin.
//!
//! The feed is the GitHub release's `latest.json`; the plugin verifies the
//! minisign signature of every downloaded artifact before the NSIS installer
//! runs, so a compromised mirror or DNS cannot ship code. The plugin enables
//! reqwest's `system-proxy` feature, which keeps the update path working
//! behind local proxies where plain `ureq` failed (AGENTS.md pitfall 38).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_updater::{Update, UpdaterExt};

static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// The frontend translates the class; the raw error stays after the `|` for
/// the console and bug reports.
fn classify(err: &tauri_plugin_updater::Error) -> &'static str {
    use tauri_plugin_updater::Error as E;
    match err {
        E::Minisign(_) | E::Base64(_) | E::SignatureUtf8(_) => "signature",
        E::Reqwest(_) | E::Network(_) => "network",
        // The server answered but the feed is missing, malformed, or has no
        // artifact for this platform - a proxy message would mislead here.
        E::ReleaseNotFound | E::TargetNotFound(_) | E::TargetsNotFound(_) => "feed",
        E::Io(_)
        | E::BinaryNotFoundInArchive
        | E::InvalidUpdaterFormat
        | E::TempDirNotFound
        | E::TempDirNotOnSameMountPoint => "install",
        #[cfg(target_os = "windows")]
        E::Extract(_) => "install",
        _ => "other",
    }
}

fn updater_error(err: tauri_plugin_updater::Error) -> String {
    format!("{}|{}", classify(&err), err)
}

fn build_updater(app: &tauri::AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    app.updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(updater_error)
}

async fn find_update(app: &tauri::AppHandle) -> Result<Option<Update>, String> {
    build_updater(app)?.check().await.map_err(updater_error)
}

#[tauri::command]
pub async fn update_check(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();
    let found = find_update(&app).await?;
    Ok(match found {
        Some(update) => UpdateInfo {
            available: true,
            current_version,
            version: Some(update.version),
            notes: update.body,
            date: update.date.map(|d| d.to_string()),
        },
        None => UpdateInfo {
            available: false,
            current_version,
            version: None,
            notes: None,
            date: None,
        },
    })
}

#[tauri::command]
pub async fn update_install(app: tauri::AppHandle) -> Result<(), String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("install|an update is already being installed".into());
    }
    let result = install_inner(&app).await;
    if result.is_err() {
        INSTALLING.store(false, Ordering::SeqCst);
    }
    result
}

async fn install_inner(app: &tauri::AppHandle) -> Result<(), String> {
    let Some(update) = find_update(app).await? else {
        return Err("noupdate|the release feed no longer announces an update".into());
    };
    let handle = app.clone();
    let mut downloaded: u64 = 0;
    let installing = handle.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = handle.emit("update-progress", UpdateProgress { downloaded, total });
            },
            move || {
                let _ = installing.emit("update-installing", ());
            },
        )
        .await
        .map_err(updater_error)?;
    // Windows: the plugin launches the NSIS installer and exits this process
    // itself (the installer's /R flag restarts the app). macOS and Linux get
    // the new bundle in place and need an explicit relaunch. `restart()`
    // diverges (`-> !`), so it must be the *tail* of a branch rather than a
    // statement followed by `Ok(())`: anything after a diverging call is
    // unreachable code, and `-D warnings` rejects it on those two platforms.
    if cfg!(target_os = "windows") {
        Ok(())
    } else {
        app.restart()
    }
}
