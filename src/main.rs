//! # Awful Jade CLI Application
//!
//! This is the main module for the Awful Jade CLI application. It handles the initialization,
//! configuration loading, and command execution based on user input from the command line.

// Importing necessary modules and libraries
#![feature(ascii_char)]
extern crate diesel;

use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestUserMessage};
use awful_aj::brain::Brain;
use awful_aj::vector_store::VectorStore;
use awful_aj::{api, commands, config, template};
use clap::Parser;
use directories::ProjectDirs;
use once_cell::sync::OnceCell;
use std::{env, error::Error, fs, path::PathBuf, vec};
use tracing::{debug, info};

// A static OnceCell to hold the tracing subscriber, ensuring it is only initialized once.
static TRACING: OnceCell<()> = OnceCell::new();

/// Main Function
///
/// Initializes tracing and the asynchronous runtime, then runs the application.
/// Any errors encountered during the run are propagated and displayed before exiting.
///
/// # Returns
/// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
fn main() -> Result<(), Box<dyn Error>> {
    initialize_tracing();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run()).unwrap();
    Ok(())
}

/// Initialize Tracing
///
/// Sets up the tracing subscriber for the application. This is used for logging
/// and is only initialized once, thanks to the `OnceCell` holding it.
fn initialize_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt::init();
    });
}

/// Run Function
///
/// The core of the application, executed asynchronously. This function is responsible for
/// parsing the command-line arguments, loading the configuration, and dispatching the
/// commands to their appropriate handlers. Errors are propagated to the `main` function.
///
/// # Returns
/// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
async fn run() -> Result<(), Box<dyn Error>> {
    let cli = commands::Cli::parse();

    match cli.command {
        commands::Commands::Ask {
            question,
            template,
            session,
        } => {
            debug!("Entering ask mode");
            let config_path = determine_config_path()?;
            let mut jade_config = config::load_config(config_path.to_str().unwrap())?;

            if session.is_some() {
                jade_config
                    .ensure_conversation_and_config(&session.unwrap())
                    .await?;
            }

            handle_ask_command(jade_config, question, template).await?;
        }
        commands::Commands::Interactive { template, session } => {
            debug!("Entering interactive mode");
            let config_path = determine_config_path()?;
            let mut jade_config = config::load_config(config_path.to_str().unwrap())?;

            if session.is_some() {
                jade_config
                    .ensure_conversation_and_config(&session.unwrap())
                    .await?;
            }

            handle_interactive_command(jade_config, template).await?;
        }
        commands::Commands::Init => {
            debug!("Initializing configuration");
            init()?;
        }
    }

    Ok(())
}

/// Handle Ask Command
///
/// Processes the 'ask' command. Loads a template and the user's question (or a default one)
/// and forwards them to the API for processing. The result is then handled as per the application's
/// design.
///
/// # Parameters
/// - `jade_config: config::AwfulJadeConfig`: The configuration for Awful Jade
/// - `question: Option<String>`: The question to be asked, or None to use a default question
///
/// # Returns
/// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
async fn handle_ask_command(
    jade_config: config::AwfulJadeConfig,
    question: Option<String>,
    template_name: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let template_name = template_name.unwrap_or_else(|| "simple_question".to_string());
    let template = template::load_template(&template_name).await?;
    let question = question.unwrap_or_else(|| "What is the meaning of life?".to_string());

    if let Some(the_session_name) = jade_config.session_name.clone() {
        let digest = sha256::digest(&the_session_name);
        let vector_store_name = format!("{}_vector_store.yaml", digest);
        let vector_store_path = config_dir()?.join(vector_store_name);
        let vector_store_string = fs::read_to_string(&vector_store_path);

        let mut vector_store: VectorStore = if vector_store_string.is_ok() {
            serde_yaml::from_str(&vector_store_string.unwrap())?
        } else {
            VectorStore::new(384, jade_config.session_name.clone().unwrap())?
        };

        let max_brain_token_percentage = 0.25;
        let max_brain_tokens =
            (max_brain_token_percentage * jade_config.context_max_tokens as f32) as u16;

        let mut brain = Brain::new(max_brain_tokens, &template);

        api::ask(
            &jade_config,
            question,
            &template,
            Some(&mut vector_store),
            Some(&mut brain),
        )
        .await?;

        let _res = vector_store.serialize(
            &vector_store_path,
            jade_config.session_name.clone().unwrap(),
        );
    } else {
        api::ask(&jade_config, question, &template, None, None).await?;
    }

    Ok(())
}

/// Handle Interactive Command
///
/// Manages the 'interactive' command. Sets up and enters the interactive mode, allowing the
/// user to engage in a conversation with the AI model. The conversation can be named, and the
/// vectors are stored for retrieval.
///
/// # Parameters
/// - `jade_config: config::AwfulJadeConfig`: The configuration for Awful Jade
/// - `name: Option<String>`: The name of the conversation, or None to use a default name
///
/// # Returns
/// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
async fn handle_interactive_command(
    jade_config: config::AwfulJadeConfig,
    template_name: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let template_name = template_name.unwrap_or_else(|| "simple_question".to_string());
    let template = template::load_template(&template_name).await?;

    let the_session_name = jade_config.session_name.clone().unwrap();
    let digest = sha256::digest(&the_session_name);
    let vector_store_name = format!("{}_vector_store.yaml", digest);
    let vector_store_path = config_dir()?.join(vector_store_name);
    let vector_store_string = fs::read_to_string(&vector_store_path);

    let vector_store: VectorStore = if vector_store_string.is_ok() {
        serde_yaml::from_str(&vector_store_string.unwrap())?
    } else {
        VectorStore::new(384, jade_config.session_name.clone().unwrap())?
    };

    let max_brain_token_percentage = 0.25;
    let max_brain_tokens =
        (max_brain_token_percentage * jade_config.context_max_tokens as f32) as u16;
    let brain = Brain::new(max_brain_tokens, &template);
    api::interactive_mode(&jade_config, vector_store, brain, &template).await
}

/// Determine Config Path
///
/// Decides the path for the configuration file. If the application is in a test environment,
/// it loads the config from the project directory. Otherwise, it uses the user's config directory.
/// The distinction ensures that tests do not interfere with a user's actual configuration.
///
/// # Returns
/// - `Result<PathBuf, Box<dyn Error>>`: The path to the configuration file or an error
fn determine_config_path() -> Result<PathBuf, Box<dyn Error>> {
    if env::var("IN_TEST_ENVIRONMENT").is_ok() {
        Ok(env::current_dir()?.join("config.yaml")) // Test environment
    } else {
        Ok(config_dir()?.join("config.yaml")) // User's config directory
    }
}

use async_openai::types::ChatCompletionRequestSystemMessage;
use async_openai::types::ChatCompletionRequestSystemMessageContent;
use async_openai::types::ChatCompletionRequestUserMessageContent;
/// Initialization Function
///
/// Handles the 'init' command. It is responsible for creating the necessary directories and
/// files, and writing the default configuration and templates into them. It ensures that the
/// application is ready for use, with all required setups completed.
///
/// # Returns
/// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
fn init() -> Result<(), Box<dyn Error>> {
    let config_dir = config_dir()?;
    let path = config_dir.join("templates");
    info!("Creating template config directory: {}", path.display());
    fs::create_dir_all(path.clone())?;

    let template_path = config_dir.join("templates/simple_question.yaml");
    info!("Creating template file: {}", template_path.display());
    let user_message = ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
        content: ChatCompletionRequestUserMessageContent::Text(
            "How do I read a file in Rust?".to_string(),
        ),
        name: None,
    });

    let system_message_content = "Use `std::fs::File` and `std::io::Read` in Rust to read a file:
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
    .to_string();

    let system_message = ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
        content: ChatCompletionRequestSystemMessageContent::Text(system_message_content),
        name: None,
    });

    let template = template::ChatTemplate {
        system_prompt: "You are Awful Jade, a helpful AI assistant programmed by Awful Security."
            .to_string(),
        messages: vec![user_message, system_message],
        response_format: None,
        pre_user_message_content: None,
        post_user_message_content: None,
    };
    let template_yaml = serde_yaml::to_string(&template)?;
    fs::write(template_path, template_yaml)?;
    // Create the default template
    create_default_template(&path)?;

    let config_path = config_dir.join("config.yaml");
    info!("Creating config file: {}", config_path.display());
    let config = config::AwfulJadeConfig {
        api_base: "http://localhost:5001/v1".to_string(),
        api_key: "CHANGEME".to_string(),
        model: "mistrel-7b-openorca".to_string(),
        context_max_tokens: 8192,
        assistant_minimum_context_tokens: 2048,
        stop_words: vec!["\n<|im_start|>".to_string(), "<|im_end|>".to_string()],
        session_db_url: "aj.db".to_string(),
        session_name: None,
        should_stream: None,
    };
    let config_yaml = serde_yaml::to_string(&config)?;
    fs::write(config_path, config_yaml)?;

    Ok(())
}

/// Create Default Template
///
/// Generates the default chat template during the initialization process. It writes a predefined
/// template to a file, ensuring that there's a starting point for the user to engage with the AI.
///
/// # Parameters
/// - `templates_dir: &Path`: The directory where the template will be stored
///
/// # Returns
/// - `Result<(), Box<dyn Error>>`: Result type indicating success or error
fn create_default_template(templates_dir: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let default_template_path = templates_dir.join("default.yaml");
    info!(
        "Creating default template file: {}",
        default_template_path.display()
    );
    let default_template_content = r#"
system_prompt: "Your name is Awful Jade, you are a helpful AI assistant programmed by Awful Security."
messages: []
"#;
    fs::write(default_template_path, default_template_content)?;
    Ok(())
}

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
