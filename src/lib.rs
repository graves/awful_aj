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