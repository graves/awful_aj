# Awful Jade (`aj`) 🌲

[![Crates.io](https://img.shields.io/crates/v/awful_aj.svg)](https://crates.io/crates/awful_aj)
[![Docs.rs](https://docs.rs/awful_aj/badge.svg)](https://docs.rs/awful_aj)

**Awful Jade** (aka **`aj`**) is your command-line sidekick for working with Large Language Models (LLMs).  

Think of it as an _LLM Swiss Army knife with the best intentions_ 😇.

> Ask questions, run interactive sessions, sanitize messy OCR book dumps, synthesize exam questions, all without leaving your terminal.

It's built in Rust for speed, safety, and peace of mind. 🦀

---

```
λ aj --help
Awful Jade – a CLI for local LLM tinkering with memories, templates, and vibes.

Usage: aj <COMMAND>

Commands:
  ask          Ask a single question and print the assistant's response
  interactive  Start an interactive REPL-style conversation
  init         Initialize configuration and default templates in the platform config directory
  reset        Reset the database to a pristine state
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

---

![Awful Jade CLI tool logo](aj.jpeg)

---

## ✨ Features

- **Ask the AI**: Run `aj ask "question"` and get answers powered by your configured model.
- **Interactive Mode**: A REPL-style conversation with memory & vector search (your AI "remembers" past context).
- **RAG Support**: Load documents for context-aware responses with `--rag` flag. Automatic chunking, caching, and retrieval.
- **Pretty Printing**: Beautiful markdown rendering with syntax highlighting for code blocks (`--pretty` flag).
- **Progress Indicators**: Real-time feedback with spinners for API calls, memory search, and model loading.
- **Vector Store**: Uses HNSW + sentence embeddings to remember what you've said before. Basically, your AI gets a brain. 🧠
- **Brains with Limits**: Keeps only as many tokens as you allow. When full, it forgets the oldest stuff. (Like you after 3 AM pizza.)
- **Config & Templates**: YAML-driven configs and prompt templates. Customize everything, break nothing.
- **Auto-downloads embeddings model**: Uses Candle (pure Rust ML framework) to automatically download the `all-MiniLM-L6-v2` BERT model from HuggingFace Hub when needed.
- **Pure Rust**: No Python dependencies! Everything runs in pure Rust using Candle for ML inference.
- **Thread-Safe**: All core types (`Brain`, `VectorStore`, `SessionMessages`) are `Send + Sync`, enabling use with tokio and concurrent workloads.

---

## 📦 Installation

From [crates.io](https://crates.io/crates/awful_aj):

```bash
cargo install awful_aj
```

This gives you the `aj` binary.

Requirements:
- Rust (use rustup if you don't have it).
  
The embeddings model (`all-MiniLM-L6-v2`) will be downloaded automatically from HuggingFace Hub to your system's cache directory when first needed. Models are cached using the `hf-hub` crate, typically at:
- macOS: `~/.cache/huggingface/hub/`
- Linux: `~/.cache/huggingface/hub/`
- Windows: `C:\Users\YOU\AppData\Local\huggingface\hub\`

---

## 👷🏽‍♀️ Setup

No special setup required! Just install with `cargo install awful_aj` and you're ready to go.

The embeddings model will be downloaded automatically from HuggingFace Hub the first time you use a feature that requires it.

---

## 🚀 Usage

### 1. Initialize

Create default configs, templates, and database:

```
aj init
```

This will generate:
- `config.yaml` with sensible defaults
- `templates/default.yaml` and `templates/simple_question.yaml`
- A SQLite database (`aj.db`) for sessions in your config directory

**Options:**
- `--overwrite`: Force overwrite existing config, templates, and database files

**Example:**
```bash
aj init --overwrite  # Reinitialize everything from scratch
```

---

### 2. Ask a Question

```
aj ask "Is Bibi really from Philly?"
```

You'll get a colorful, model-dependent answer in **yellow** (or **dark gray** if the model uses `<think>` tags for reasoning).

**Options:**
- `-t, --template <NAME>`: Use a specific template (e.g., `simple_question`)
- `-s, --session <NAME>`: Save to a named session for context retention
- `-o, --one-shot`: Ignore any session configured in config.yaml (force standalone prompt)
- `-r, --rag <FILES>`: Comma-separated list of files to use as RAG context
- `-k, --rag-top-k <N>`: Number of RAG chunks to retrieve (default: 3)
- `-p, --pretty`: Enable markdown rendering with syntax highlighting

**Examples:**
```bash
aj ask "What is HNSW?"
aj ask -t simple_question "Explain Rust lifetimes"
aj ask -s my-session "Remember this: I like pizza"
aj ask -o "What's the weather?" # Ignores session from config
aj ask -r docs.txt,notes.md -k 5 "Summarize the key points"
aj ask -p "Explain this code" # Pretty markdown output
aj ask -r docs/ -p -s project "What does this project do?"
```

![aj ask command](./bibi.gif)

---

### 3. Interactive Mode

Talk with the AI in an interactive REPL:

```
aj interactive
```

Supports memory via the vector store, so it won't immediately forget your name.
_(Unlike your barista.)_

**Colors:**
- Your input appears in **blue**
- Assistant responses appear in **yellow**
- Model reasoning (in `<think>` tags) appears in **dark gray**

**Options:**
- `-t, --template <NAME>`: Use a specific template
- `-s, --session <NAME>`: Use a named session
- `-r, --rag <FILES>`: Comma-separated list of files for RAG context (loaded once for entire session)
- `-k, --rag-top-k <N>`: Number of RAG chunks to retrieve (default: 3)
- `-p, --pretty`: Enable markdown rendering with syntax highlighting for all responses

**Examples:**
```bash
aj interactive
aj interactive -s my-session
aj interactive -t reading_buddy -s book-club
aj interactive -r docs/ -p -s project  # Interactive with RAG and pretty output
```

---

### 4. Reset Database

Start fresh by resetting the database to a pristine state:

```
aj reset
```

This drops all sessions, messages, and recreates the schema. Useful when you want a clean slate.

**Aliases:** `aj r`

---

### 5. Configuration

Edit your config at:

```
~/.config/aj/config.yaml   # Linux
~/Library/Application Support/com.awful-sec.aj/config.yaml   # macOS
```

Example:

```yaml
api_base: "http://localhost:1234/v1"
api_key: "CHANGEME"
model: "jade_qwen3_4b_mlx"
context_max_tokens: 8192
assistant_minimum_context_tokens: 2048
stop_words:
  - "<|im_end|>\\n<|im_start|>"
  - "<|im_start|>\n"
session_db_url: "/Users/you/Library/Application Support/com.awful-sec.aj/aj.db"
session_name: "default"  # Set to null for no session persistence
should_stream: true       # Enable streaming responses
temperature: 0.7          # Sampling temperature (0.0-2.0, optional)
```

---

### 6. RAG (Retrieval-Augmented Generation)

Load documents as context for your queries:

```bash
aj ask -r document.txt "What are the main points?"
aj ask -r docs/,notes.md -k 5 "Summarize these files"
```

**How it works:**
- Documents are automatically chunked (512 tokens, 128 overlap)
- Chunks are embedded and cached in `~/.config/aj/rag_cache/` (or platform equivalent)
- Top-k most relevant chunks are retrieved and injected into context
- Cache is reused for faster subsequent queries on the same files

**Cache location:**
- macOS: `~/Library/Application Support/com.awful-sec.aj/rag_cache/`
- Linux: `~/.config/aj/rag_cache/`
- Windows: `%APPDATA%\com.awful-sec\aj\rag_cache\`

---

### 7. Templates

Templates are YAML files in your config directory.
Here's a baby template:

```yaml
system_prompt: "You are Awful Jade, a helpful AI assistant programmed by Awful Security."
messages: []
response_format: null
pre_user_message_content: null
post_user_message_content: null
```

Add more, swap them in with `-t <name>` or `--template <name>`.

---

## 🧠 How it Works

- **Brain**: Token-budgeted working memory with FIFO eviction. Keeps memories in a deque, trims when it gets too wordy.
- **VectorStore**: Embeds your inputs using `all-MiniLM-L6-v2` via Candle (pure Rust ML), saves to HNSW index for semantic search.
- **RAG System**: Intelligent document chunking (512 tokens, 128 overlap), embedding caching, and k-nearest neighbor retrieval.
- **Pretty Printing**: Markdown rendering with syntax highlighting for 100+ languages using Syntect.
- **Progress UI**: Real-time spinners and feedback using Indicatif (API calls, memory search, model loading).
- **Candle**: Pure Rust ML framework from HuggingFace. Automatically downloads and caches models from HuggingFace Hub.
- **Config**: YAML-based, sane defaults, easy to tweak.
- **Templates**: Prompt engineering without copy-pasting into your terminal like a caveman.
- **No Python**: Everything runs in pure Rust with no external ML runtime dependencies.

---

## 🧑‍💻 Development

Clone, hack, repeat:

```
git clone https://github.com/graves/awful_aj.git
cd awful_aj
cargo build
```

Run tests:

```
cargo test
```

## ✨ What's New in v0.4.0

### 🚀 Major Features
- **Enhanced RAG Performance**: Improved chunking algorithm with better overlap handling
- **Pretty Printing**: Added markdown rendering with syntax highlighting for 100+ languages
- **Session Management**: Better token budgeting with FIFO eviction
- **Cross-platform**: Improved Windows support with proper path handling
- **Memory Efficiency**: Optimized HNSW indexing for faster semantic search

### 🔧 Improvements
- **Better Error Messages**: More descriptive error reporting with actionable solutions
- **Configuration Validation**: Enhanced config parsing with helpful error messages
- **Caching**: Improved RAG cache management and cleanup
- **Documentation**: Comprehensive troubleshooting guide and quick reference

### 🐛 Bug Fixes
- **Fixed session persistence issues** on Windows
- **Resolved embedding download failures** with better error handling
- **Fixed memory leaks** in long-running interactive sessions
- **Improved template loading** with better validation

---

## 🔧 Troubleshooting

Common issues and solutions are covered in our [troubleshooting guide](docs/src/troubleshooting.md).

Quick fixes for common problems:
- **Model not found**: Check model name in `config.yaml`
- **Embedding download fails**: Verify internet connection and clear cache
- **Session errors**: Check directory permissions
- **API connection**: Verify URL includes port and server is running

## 📋 Quick Reference

| Command | Purpose | Key Flags |
|---------|---------|-----------|
| `aj ask` | One-shot Q&A | `-t`, `-r`, `-p`, `-s` |
| `aj interactive` | Chat REPL | `-t`, `-r`, `-p`, `-s` |
| `aj init` | Setup configuration | `--overwrite` |
| `aj reset` | Clear database | - |
| `aj --help` | Show help | `-h`, `-V` |

## 🤝 Contributing

PRs welcome!
Bugs, docs, new templates, vector hacks—bring it on.
But remember: with great power comes great YAML.

---

## 📜 License

[CC-BY-SA-4.0](https://creativecommons.org/licenses/by-sa/4.0/) (Creative Commons Attribution-ShareAlike 4.0 International)

Share and adapt freely, but give credit and share alike. Don't blame us when your AI remembers your browser history.

---

💡 Awful Jade: bad name, good brain.

---
