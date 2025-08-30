//! # Awful Jade (library root)
//!
//! This crate provides the core plumbing for the **Awful Jade** CLI and library:
//! - High-level chat API bindings (`api`).
//! - Lightweight long-term memory & context management (`brain`, `vector_store`).
//! - CLI parsing & commands (`commands`).
//! - Configuration & DB integration (`config`, `models`, `schema`, `session_messages`).
//! - Prompt/template handling (`template`).
//!
//! In addition, this module exposes utilities for:
//! - Discovering the per-platform configuration directory ([`config_dir`]).
//! - Managing a local copy of the **`all-mini-lm-l12-v2`** sentence-embedding model,
//!   including **download/unzip** and **cross-platform linking** helpers so the CLI can find
//!   `./all-mini-lm-l12-v2` without bundling the model into the crate (which would break
//!   crates.io size limits).
//!
//! ## Embedding model layout & discovery
//! By default, the BERT sentence embedding model is expected under your per-platform config directory, e.g.:
//!
//! - macOS: `~/Library/Application Support/com.awful-sec.aj/all-mini-lm-l12-v2`  
//! - Linux (XDG): `~/.config/aj/all-mini-lm-l12-v2`
//! - Windows: `C:\Users\<you>\AppData\Roaming\aj\all-mini-lm-l12-v2`
//!
//! Two helper functions exist:
//!
//! - [`ensure_all_mini_present`] — purely **local** resolution. It accepts an optional CLI override
//!   and will optionally create a **link in the current working directory** so `./all-mini-lm-l12-v2`
//!   resolves for tools that expect it there. It does **not** download anything.
//! - [`ensure_all_mini`] — **networked** helper. If the model is missing, it downloads a zip from
//!   your configured URL and extracts it into the config directory.
//!
//! ## Windows links
//! On Windows you can enable directory **junctions** instead of symlinks by compiling with the
//! `windows-junction` feature. Junctions don’t require admin/Developer Mode, unlike symlinks.
//!
//! ```text
//! # Unix: symlink
//! # Windows default: symlink_dir (requires admin/Developer Mode)
//! # Windows + feature "windows-junction": create a junction instead of a symlink
//! ```
//!
//! ## Modules
//! - [`api`], [`brain`], [`commands`], [`config`], [`models`], [`schema`],
//!   [`session_messages`], [`template`], [`vector_store`]

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

/// Return the per-platform configuration directory used by Awful Jade.
///
/// This uses [`directories::ProjectDirs`] with the application triple
/// `("com", "awful-sec", "aj")`, so you get the right place on each OS
/// (e.g., `~/Library/Application Support/com.awful-sec.aj` on macOS).
///
/// The directory is **not** created by this function; callers that need it should
/// create it with `fs::create_dir_all`.
///
/// # Errors
/// Returns an error if the platform configuration directory cannot be determined
/// (which is rare but possible in heavily sandboxed environments).
///
/// # Examples
/// ```rust
/// let cfg = awful_aj::config_dir().expect("has a config dir");
/// println!("config at {}", cfg.display());
/// ```
pub fn config_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let proj_dirs = ProjectDirs::from("com", "awful-sec", "aj")
        .ok_or("Unable to determine config directory")?;
    let config_dir = proj_dirs.config_dir().to_path_buf();

    Ok(config_dir)
}

/// Internal: the **default** on-disk location for the sentence-embedding model.
///
/// This is `config_dir()/all-mini-lm-l12-v2`. The directory may or may not exist.
fn default_model_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(crate::config_dir()?.join("all-mini-lm-l12-v2"))
}

/// Internal: the path where a **CWD link** would live (if created).
///
/// This is `./all-mini-lm-l12-v2` under the current working directory.
fn cwd_model_link() -> io::Result<PathBuf> {
    Ok(std::env::current_dir()?.join("all-mini-lm-l12-v2"))
}

/// Internal: does `p` look like a **non-empty directory**?
fn exists_nonempty_dir(p: &Path) -> bool {
    p.is_dir()
        && fs::read_dir(p)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false)
}

// --- Cross-platform link helpers ---------------------------------------------------------------

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
    // Requires admin or Developer Mode to create directory symlinks
    wfs::symlink_dir(src, dst)
}

#[cfg(all(windows, feature = "windows-junction"))]
fn link_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() {
        let _ = fs::remove_dir_all(dst);
        let _ = fs::remove_file(dst);
    }
    // Create a junction instead of a symlink (no admin required)
    junction::create(src, dst).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Resolve a usable `all-mini-lm-l12-v2` directory **without downloading**.
///
/// This function picks a model directory from (in priority order):
/// 1. An explicit override path (`cli_override`) — typically from a CLI flag or env var.
/// 2. A **CWD** entry at `./all-mini-lm-l12-v2`.
/// 3. The default location under the app config dir: `config_dir()/all-mini-lm-l12-v2`.
///
/// If the effective directory is found and `no_cwd_link == false`, a **link** is created in
/// the current working directory so that tools that expect `./all-mini-lm-l12-v2` will work
/// without copying the model.
///
/// This is useful when you’ve installed the model once under the config dir and want to make it
/// visible to multiple projects without duplicating ~135MB of weights.
///
/// > **Note:** If you need the function that *downloads* the model when missing,
/// > see [`ensure_all_mini`].
///
/// # Parameters
/// - `cli_override`: Optional explicit model directory to use.
/// - `no_cwd_link`: If `true`, skip creating a CWD link even when possible.
///
/// # Returns
/// The **real** directory where the model lives (not the link path).
///
/// # Errors
/// - The override path was provided but does not exist or is empty.
/// - No valid model directory could be found in the known locations.
/// - Link creation failed (e.g., missing privileges for Windows symlinks when the
///   `windows-junction` feature is **not** used).
///
/// # Examples
/// ```rust
/// use std::path::PathBuf;
///
/// // Use default resolution and create a CWD link if possible
/// let model = awful_aj::ensure_all_mini_present(None, false).expect("model present");
/// println!("model at {}", model.display());
///
/// // Respect an explicit override and do NOT create a CWD link
/// let override_dir = PathBuf::from("/opt/models/all-mini-lm-l12-v2");
/// let model = awful_aj::ensure_all_mini_present(Some(override_dir), true).expect("found");
/// ```
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

    // 4) Nothing found — instruct caller how to provide it
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

/// Ensure the `all-mini-lm-l12-v2` model exists under your config dir; download if missing.
///
/// If `config_dir()/all-mini-lm-l12-v2` is **non-empty**, this is a no-op and returns the path.
/// Otherwise it:
/// 1. Creates the config directory (if needed).
/// 2. Downloads a zip from the configured URL (currently `http://awfulsec.com/bigfiles/all-mini-lm-l12-v2.zip`).
/// 3. Extracts the archive into the **config dir** (the zip is expected to contain the
///    `all-mini-lm-l12-v2/` folder).
///
/// This function does **not** create a CWD link. If you need `./all-mini-lm-l12-v2`, call
/// [`ensure_all_mini_present`] afterwards to create a link.
///
/// # Returns
/// The absolute path to `config_dir()/all-mini-lm-l12-v2`.
///
/// # Errors
/// - Network failures (download errors).
/// - Invalid/unsupported zip archive.
/// - Filesystem errors while creating directories or extracting files.
///
/// # Examples
/// ```no_run
/// # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let path = awful_aj::ensure_all_mini().await?;
/// println!("Model ready at {}", path.display());
/// # Ok(()) }
/// ```
pub async fn ensure_all_mini() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = crate::config_dir()?; // your existing helper
    let target_dir = base.join("all-mini-lm-l12-v2");

    if target_dir.is_dir() && fs::read_dir(&target_dir)?.next().is_some() {
        return Ok(target_dir);
    }

    eprintln!("Model not found, downloading to {} …", target_dir.display());

    fs::create_dir_all(&base)?;

    // Download into memory (sufficient for ~135MB; switch to streaming to a temp file if needed)
    let url = "http://awfulsec.com/bigfiles/all-mini-lm-l12-v2.zip";
    let bytes = reqwest::get(url).await?.bytes().await?;
    let reader = Cursor::new(bytes);

    // Open as zip archive
    let mut archive = zip::ZipArchive::new(reader)?;

    // Extract into config_dir (expects the archive to contain all-mini-lm-l12-v2/)
    archive.extract(&base)?;

    eprintln!("BERT model extracted to {}", target_dir.display());

    Ok(target_dir)
}
