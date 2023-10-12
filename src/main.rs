//! Main module for the Awful Jade CLI application (aj).
//!
//! This module provides the main function and auxiliary functionalities for
//! the CLI application. It handles command parsing, configuration loading, and
//! initialization, as well as invoking the appropriate functionalities based on
//! the provided command-line arguments.
//!
//! # Examples
//!
//! Running the application with the `ask` command:
//!
//! ```sh
//! cargo run -- ask "What is the meaning of life?"
//! aj ask "What is the meaning of life?"
//! ```
//!
//! Initializing the application's configuration and templates:
//!
//! ```sh
//! cargo run -- init
//! aj init
//! ```

mod api;
mod commands;
mod config;
mod template;

use clap::Parser;
use directories::ProjectDirs;
use once_cell::sync::OnceCell;
use std::{env, error::Error, fs};
use tracing::{debug, info};

static TRACING: OnceCell<()> = OnceCell::new();

fn main() -> Result<(), Box<dyn Error>> {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt::init();
    });
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run()).unwrap();
    Ok(())
}

/// Main asynchronous function of the Awful Jade CLI application.
///
/// Initializes the tracing subscriber, loads configuration, parses command-line arguments,
/// and executes the appropriate command.
///
/// # Errors
///
/// Returns an error if there is an issue loading the configuration, parsing the command-line
/// arguments, or executing the specified command.
async fn run() -> Result<(), Box<dyn Error>> {
    println!(
        "IN_TEST_ENVIRONMENT in run {}",
        env::var("IN_TEST_ENVIRONMENT").unwrap_or_default()
    );
    let config_path = if env::var("IN_TEST_ENVIRONMENT").is_ok() {
        // If we're in a test environment, load the config from the project directory
        let path = env::current_dir()?.join("config.yaml");
        println!("Loading config from: {}", path.display());
        path
    } else {
        println!("Loading config from default location");
        // Otherwise, load the config from the user's config directory
        config_dir()?.join("config.yaml")
    };

    debug!("Loading config from: {}", config_path.display());
    let jade_config = config::load_config(config_path.to_str().unwrap())?;
    debug!("Config loaded: {:?}", jade_config);
    let cli = commands::Cli::parse();

    match cli.command {
        commands::Commands::Ask { question } => {
            debug!("Asking question: {:?}", question);
            let template = template::load_template("simple_question").await?;
            let question = question.unwrap_or_else(|| "What is the meaning of life?".to_string());
            api::ask(&jade_config, question, template).await?;
        }
        commands::Commands::Init => {
            debug!("Initializing configuration");
            init()?;
        }
    }

    Ok(())
}

/// Initializes the application's configuration and templates.
///
/// Creates the necessary directories and files for the application's configuration and
/// default chat template. The configuration and template are stored in YAML format.
///
/// # Errors
///
/// Returns an error if there is an issue creating the directories or files, or
/// serializing the configuration and template to YAML.
fn init() -> Result<(), Box<dyn Error>> {
    let config_dir = config_dir()?;
    let path = config_dir.join("templates");
    info!("Creating template config directory: {}", path.display());
    fs::create_dir_all(path)?;

    let template_path = config_dir.join("templates/simple_question.yml");
    info!("Creating template file: {}", template_path.display());
    let template = template::ChatTemplate {
        system_prompt: "You are Awful Jade, a helpful AI assistant programmed by Awful Security."
            .to_string(),
        messages: vec![
            async_openai::types::ChatCompletionRequestMessage {
                role: async_openai::types::Role::User,
                content: Some("How do I read a file in Rust?".to_string()),
                name: None,
                function_call: None,
            },
            async_openai::types::ChatCompletionRequestMessage {
                role: async_openai::types::Role::Assistant,
                content: Some(
                    "Use `std::fs::File` and `std::io::Read` in Rust to read a file:
```rust
use std::fs::File;
use std::io::{self, Read};

fn main() -> io::Result<()> {
    let mut file = File::open(\"file.txt\")?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    println!(\"{}\", content);
    Ok(())
}
```"
                    .to_string(),
                ),
                name: None,
                function_call: None,
            },
        ],
    };
    let template_yaml = serde_yaml::to_string(&template)?;
    fs::write(template_path, template_yaml)?;

    let config_path = config_dir.join("config.yaml");
    info!("Creating config file: {}", config_path.display());
    let config = config::AwfulJadeConfig {
        api_base: "http://localhost:5001/v1".to_string(),
        api_key: "CHANGEME".to_string(),
        model: "mistrel-7b-openorca".to_string(),
    };
    let config_yaml = serde_yaml::to_string(&config)?;
    fs::write(config_path, config_yaml)?;

    Ok(())
}

/// Retrieves the configuration directory for the application.
///
/// Utilizes the `directories` crate to determine the appropriate configuration directory
/// based on the operating system's conventions.
///
/// # Errors
///
/// Returns an error if unable to determine the configuration directory.
pub fn config_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let proj_dirs = ProjectDirs::from("com", "awful-security", "aj")
        .ok_or("Unable to determine config directory")?;
    Ok(proj_dirs.config_dir().to_path_buf())
}

