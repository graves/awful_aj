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
//!
//! ## Embedding model
//! The sentence embedding model (`all-MiniLM-L6-v2`) is automatically downloaded from
//! HuggingFace Hub by the Candle framework when first used. Models are cached in the
//! standard HuggingFace cache directory.
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

/// Return the per-platform configuration directory used by Awful Jade.
///
/// This uses [`directories::ProjectDirs`] with the application triple
/// `(\"com\", \"awful-sec\", \"aj\")`, so you get the right place on each OS
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
