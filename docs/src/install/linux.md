# Install on Linux 🐧

## ✅ Requirements
- Rust (via [rustup](https://rustup.rs/))

That's it! No Python, no conda, no PyTorch needed. Everything runs in pure Rust. 🦀

## 📦 Install from crates.io

```shell
cargo install awful_aj
```

## 🏗️ Initialize

Create default config, templates, and database:

```shell
aj init
```

This creates:
- `~/.config/aj/config.yaml`
- `~/.config/aj/templates/`
- `~/.config/aj/aj.db` (SQLite database)

## 🤖 First Run

On first use, `aj` will automatically download the `all-MiniLM-L6-v2` embeddings model from HuggingFace Hub to:

`~/.cache/huggingface/hub/`

This happens automatically when you first use a feature that requires embeddings (like sessions with memory).

## ✅ You're ready!

Try it out:

```shell
aj ask "Hello from Linux!"
```

## 🔄 Reset Database

If you ever need to start fresh:

```shell
aj reset  # Drops all sessions and messages, recreates schema
