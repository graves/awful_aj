use directories::ProjectDirs;
use std::error::Error;

pub mod api;
pub mod brain;
pub mod commands;
pub mod config;
pub mod models;
pub mod schema;
pub mod session_messages;
pub mod template;
pub mod vector_store;

use std::{
    fs,
    io::{self, Cursor},
    path::{Path, PathBuf},
};

/// Configuration Directory Retrieval
///
/// Uses the `directories` crate to fetch the appropriate configuration directory based on the
/// operating system. This ensures compatibility and adherence to the OS's directory structure
/// and conventions.
///
/// # Returns
/// - `Result<PathBuf, Box<dyn Error>>`: The path to the configuration directory or an error
pub fn config_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let proj_dirs = ProjectDirs::from("com", "awful-sec", "aj")
        .ok_or("Unable to determine config directory")?;
    let config_dir = proj_dirs.config_dir().to_path_buf();

    Ok(config_dir)
}

fn default_model_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // ~/.config/aj/all-mini-lm-l12-v2 (on Linux/macOS) or the platform’s config dir
    Ok(crate::config_dir()?.join("all-mini-lm-l12-v2"))
}

fn cwd_model_link() -> io::Result<PathBuf> {
    Ok(std::env::current_dir()?.join("all-mini-lm-l12-v2"))
}

fn exists_nonempty_dir(p: &Path) -> bool {
    p.is_dir()
        && fs::read_dir(p)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false)
}

// --- Cross-platform link helpers ---

#[cfg(unix)]
fn link_dir(src: &Path, dst: &Path) -> io::Result<()> {
    use std::os::unix::fs as ufs;
    if dst.exists() {
        // remove stale symlink/file; if it's a directory, try remove_dir_all
        let _ = fs::remove_file(dst);
        let _ = fs::remove_dir_all(dst);
    }
    ufs::symlink(src, dst)
}

#[cfg(all(windows, not(feature = "windows-junction")))]
fn link_dir(src: &Path, dst: &Path) -> io::Result<()> {
    use std::os::windows::fs as wfs;
    if dst.exists() {
        let _ = fs::remove_dir_all(dst);
        let _ = fs::remove_file(dst);
    }
    wfs::symlink_dir(src, dst) // requires admin or Developer Mode
}

#[cfg(all(windows, feature = "windows-junction"))]
fn link_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() {
        let _ = fs::remove_dir_all(dst);
        let _ = fs::remove_file(dst);
    }
    junction::create(src, dst).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Ensure awful_aj can find `./all-mini-lm-l12-v2` without bundling it.
/// Returns the **real** model directory that will be used.
pub fn ensure_all_mini_present(
    cli_override: Option<PathBuf>,
    no_cwd_link: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cwd_link = cwd_model_link()?;

    // 1) If caller provided a path (flag or env), prefer it
    if let Some(dir) = cli_override {
        if !exists_nonempty_dir(&dir) {
            return Err(format!(
                "--model-dir points to a non-existent/empty directory: {}",
                dir.display()
            )
            .into());
        }
        if !no_cwd_link && !exists_nonempty_dir(&cwd_link) {
            link_dir(&dir, &cwd_link)?;
        }
        return Ok(dir);
    }

    // 2) If CWD already has the folder/symlink, use it as-is
    if exists_nonempty_dir(&cwd_link) {
        return Ok(cwd_link);
    }

    // 3) Fallback to config_dir()/all-mini-lm-l12-v2
    let cfg_dir = default_model_dir()?;
    if exists_nonempty_dir(&cfg_dir) {
        if !no_cwd_link {
            link_dir(&cfg_dir, &cwd_link)?;
        }
        return Ok(cfg_dir);
    }

    // 4) Nothing found — instruct user how to provide it
    Err(format!(
        "Could not locate 'all-mini-lm-l12-v2'. Provide it via:\n\
         - --model-dir /path/to/all-mini-lm-l12-v2 (or AWFUL_AJ_MODEL_DIR), or\n\
         - place it in {}, or\n\
         - put it under {}",
        cwd_link.display(),
        cfg_dir.display()
    )
    .into())
}

/// Ensure the `all-mini-lm-l12-v2` directory exists under your config_dir.
/// If missing, downloads the .zip from your URL and extracts it.
pub async fn ensure_all_mini() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = crate::config_dir()?; // your existing helper
    let target_dir = base.join("all-mini-lm-l12-v2");

    if target_dir.is_dir() && fs::read_dir(&target_dir)?.next().is_some() {
        return Ok(target_dir);
    }

    eprintln!("Model not found, downloading to {} …", target_dir.display());

    fs::create_dir_all(&base)?;

    // Download into memory (could also stream to file if size becomes too big)
    let url = "http://awfulsec.com/bigfiles/all-mini-lm-l12-v2.zip";
    let bytes = reqwest::get(url).await?.bytes().await?;
    let reader = Cursor::new(bytes);

    // Open as zip archive
    let mut archive = zip::ZipArchive::new(reader)?;

    // Extract into config_dir
    archive.extract(&base)?;

    eprintln!("BERT model extracted to {}", target_dir.display());

    Ok(target_dir)
}
