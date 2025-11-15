//! # Awful Jade - Local-First LLM Client with Semantic Memory
//!
//! **Awful Jade** is a Rust library and CLI for interacting with OpenAI-compatible
//! language model APIs with advanced features for memory management, RAG (Retrieval-
//! Augmented Generation), and conversation persistence.
//!
//! ## Key Features
//!
//! - **OpenAI-Compatible API Client**: Works with local LLMs (Ollama, LM Studio, vLLM)
//!   and cloud providers (OpenAI, Anthropic, etc.)
//! - **Semantic Memory System**: HNSW vector indexing with sentence embeddings for
//!   intelligent context retrieval
//! - **RAG Support**: Document chunking, embedding, and retrieval for grounded responses
//! - **Session Persistence**: SQLite database for conversation history and continuity
//! - **Token Budgeting**: Intelligent context window management with FIFO eviction
//! - **Streaming Responses**: Real-time token-by-token output
//! - **Pretty Printing**: Markdown rendering and syntax highlighting for code blocks
//! - **Template System**: YAML-based prompt engineering with system prompts and message seeds
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     awful_aj Library                         │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
//! │  │   Commands   │  │  API Client  │  │   Template   │      │
//! │  │  (CLI Args)  │  │  (OpenAI)    │  │   (YAML)     │      │
//! │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
//! │         │                  │                  │              │
//! │  ┌──────┴──────────────────┴──────────────────┴───────────┐ │
//! │  │              Brain (Working Memory)                     │ │
//! │  │   ┌─────────────┐  ┌─────────────┐  ┌──────────────┐  │ │
//! │  │   │  Preamble   │  │ RAG Context │  │   Memories   │  │ │
//! │  │   └─────────────┘  └─────────────┘  └──────────────┘  │ │
//! │  └──────────────────────────────────────────────────────┬─┘ │
//! │                                                          │   │
//! │  ┌───────────────────────────────────────────────────────┴─┐ │
//! │  │        Long-Term Memory (Vector Store + SQLite)         │ │
//! │  │   ┌──────────────┐           ┌──────────────┐          │ │
//! │  │   │ HNSW Index   │           │   Sessions   │          │ │
//! │  │   │ (Semantic)   │           │  (Messages)  │          │ │
//! │  │   └──────────────┘           └──────────────┘          │ │
//! │  └─────────────────────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Modules
//!
//! | Module | Purpose | Key Types |
//! |--------|---------|-----------|
//! | [`api`] | OpenAI API client and orchestration | `ask()`, `stream_response()`, `fetch_response()` |
//! | [`brain`] | Working memory with token budgeting | [`Brain`](brain::Brain), [`Memory`](brain::Memory) |
//! | [`vector_store`] | HNSW semantic search | [`VectorStore`](vector_store::VectorStore), [`SentenceEmbeddingsModel`](vector_store::SentenceEmbeddingsModel) |
//! | [`session_messages`] | Conversation persistence | [`SessionMessages`](session_messages::SessionMessages) |
//! | [`template`] | YAML prompt templates | [`ChatTemplate`](template::ChatTemplate) |
//! | [`commands`] | CLI argument parsing | [`Cli`](commands::Cli), [`Commands`](commands::Commands) |
//! | [`config`] | Configuration management | [`AwfulJadeConfig`](config::AwfulJadeConfig) |
//! | [`models`] | Database ORM models | [`Session`](models::Session), [`Message`](models::Message) |
//! | [`schema`] | Diesel schema definitions | `sessions`, `messages` tables |
//! | [`pretty`] | Terminal formatting | `print_pretty()`, [`PrettyPrinter`](pretty::PrettyPrinter) |
//!
//! ## Quick Start
//!
//! ### As a Library
//!
//! ```no_run
//! use awful_aj::{config::load_config, brain::Brain, template::ChatTemplate, api};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load configuration
//!     let config = load_config("config.yaml")?;
//!
//!     // Create template
//!     let template = ChatTemplate {
//!         system_prompt: "You are a helpful assistant.".into(),
//!         messages: vec![],
//!         response_format: None,
//!         pre_user_message_content: None,
//!         post_user_message_content: None,
//!     };
//!
//!     // Ask a question
//!     let response = api::ask(
//!         &config,
//!         "What is HNSW?".into(),
//!         &template,
//!         None, // no vector store
//!         None, // no brain
//!         false, // not pretty
//!     ).await?;
//!
//!     println!("{}", response);
//!     Ok(())
//! }
//! ```
//!
//! ### As a CLI
//!
//! ```bash
//! # Initialize configuration
//! aj init
//!
//! # Ask a question
//! aj ask "What is HNSW indexing?"
//!
//! # Interactive session
//! aj interactive -s my-project
//!
//! # RAG with documents
//! aj ask -r "docs/*.txt" -k 5 "Summarize the documentation"
//! ```
//!
//! ## Embedding Model
//!
//! The sentence embedding model (`all-MiniLM-L6-v2`) is automatically downloaded from
//! HuggingFace Hub by the Candle framework when first used. It produces 384-dimensional
//! embeddings suitable for semantic search.
//!
//! **Model Details**:
//! - Architecture: Sentence Transformer (BERT-based)
//! - Dimensions: 384
//! - Size: ~90MB
//! - Cache Location: Standard HuggingFace cache directory
//!
//! ## Configuration
//!
//! Configuration is loaded from platform-specific directories:
//!
//! - **macOS**: `~/Library/Application Support/com.awful-sec.aj/config.yaml`
//! - **Linux**: `~/.config/aj/config.yaml`
//! - **Windows**: `%APPDATA%\\com.awful-sec\\aj\\config.yaml`
//!
//! See [`config::AwfulJadeConfig`] for available settings.
//!
//! ## Memory Management
//!
//! Awful Jade uses a two-tier memory system:
//!
//! 1. **Working Memory ([`Brain`](brain::Brain))**: Token-budgeted FIFO queue
//!    - Preamble (system prompt, always included)
//!    - RAG context (document chunks)
//!    - Recent memories (conversation turns)
//!    - Eviction when context exceeds `context_max_tokens`
//!
//! 2. **Long-Term Memory ([`VectorStore`](vector_store::VectorStore))**: Semantic search
//!    - HNSW index for fast approximate nearest neighbor search
//!    - Euclidean distance similarity (threshold < 1.0)
//!    - Automatic embedding of evicted memories
//!
//! See [`brain`] and [`vector_store`] modules for details.
//!
//! ## RAG Pipeline
//!
//! Retrieval-Augmented Generation workflow:
//!
//! 1. **Document Loading**: Read text files from specified paths
//! 2. **Chunking**: Split into overlapping segments (512 tokens, 128 overlap)
//! 3. **Embedding**: Encode chunks with sentence transformer model
//! 4. **Indexing**: Build HNSW index for fast retrieval
//! 5. **Retrieval**: Query index with user prompt, fetch top-k chunks
//! 6. **Injection**: Add retrieved chunks to brain's preamble
//! 7. **Generation**: LLM generates response with grounded context
//!
//! See [`api::process_rag_documents`] for implementation details.
//!
//! ## Examples
//!
//! See the `examples/` directory and [`commands`] module documentation for comprehensive
//! usage examples.

use directories::ProjectDirs;
use std::error::Error;

pub mod api;
pub mod brain;
pub mod commands;
pub mod config;
pub mod models;
pub mod pretty;
pub mod schema;
pub mod session_messages;
pub mod template;
pub mod vector_store;

/// Returns the platform-specific configuration directory for Awful Jade.
///
/// This function uses [`directories::ProjectDirs`] with the application triple
/// `("com", "awful-sec", "aj")` to determine the correct configuration directory
/// for the current platform.
///
/// # Platform Paths
///
/// The returned path depends on the operating system:
///
/// - **macOS**: `~/Library/Application Support/com.awful-sec.aj`
/// - **Linux**: `~/.config/aj`
/// - **Windows**: `C:\Users\<username>\AppData\Roaming\com.awful-sec\aj`
///
/// # Important Notes
///
/// - **Directory is NOT created**: This function only returns the path. Callers
///   must create the directory using `fs::create_dir_all()` if it doesn't exist.
/// - **Used throughout the application**: Configuration files, templates, and the
///   SQLite database all live under this directory.
///
/// # Errors
///
/// Returns an error if the platform configuration directory cannot be determined.
/// This is rare but possible in heavily sandboxed environments or when the user's
/// home directory is inaccessible.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use awful_aj::config_dir;
/// use std::fs;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cfg_dir = config_dir()?;
/// println!("Config directory: {}", cfg_dir.display());
///
/// // Create the directory if it doesn't exist
/// fs::create_dir_all(&cfg_dir)?;
/// # Ok(())
/// # }
/// ```
///
/// ## Building Paths to Configuration Files
///
/// ```rust
/// use awful_aj::config_dir;
/// use std::path::PathBuf;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cfg_dir = config_dir()?;
///
/// let config_yaml = cfg_dir.join("config.yaml");
/// let templates_dir = cfg_dir.join("templates");
/// let database = cfg_dir.join("aj.db");
///
/// println!("Config file: {}", config_yaml.display());
/// println!("Templates directory: {}", templates_dir.display());
/// println!("Database: {}", database.display());
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`config::AwfulJadeConfig::load`] for loading configuration from this directory
/// - [`commands::Commands::Init`] for initializing the directory structure
pub fn config_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let proj_dirs = ProjectDirs::from("com", "awful-sec", "aj")
        .ok_or("Unable to determine config directory")?;
    let config_dir = proj_dirs.config_dir().to_path_buf();

    Ok(config_dir)
}
