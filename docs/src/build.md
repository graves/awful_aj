# Build from Source 🧱

Want to hack on `aj`? Let's go! 🧑‍💻

## 🤢 Install dependencies

All you need is Rust:

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

That's it! No Python, no conda, no PyTorch. Everything runs in pure Rust. 🦀

## 🛠️ Clone & Build

```shell
git clone https://github.com/graves/awful_aj.git
cd awful_aj
cargo build
```

## ✅ Run Tests

```shell
cargo test
```

## 🚀 Run the Local Build

```shell
cargo run -- ask "Hello!"
```

Or build in release mode for better performance:

```shell
cargo build --release
./target/release/aj ask "Hello!"
```

## 🤖 First Run with Embeddings

The first time you use a feature requiring embeddings (like sessions with memory), Candle will automatically download the `all-MiniLM-L6-v2` model from HuggingFace Hub to your cache directory:

- macOS: `~/.cache/huggingface/hub/`
- Linux: `~/.cache/huggingface/hub/`
- Windows: `C:\Users\YOU\AppData\Local\huggingface\hub\`

## 🧯 Common Troubleshooting

- **Build errors**: Make sure you have the latest stable Rust: `rustup update stable`
- **Model not downloading**: 
    - Ensure your cache directory is writable
    - Check your network connection
    - The model is downloaded on-demand when first needed
- **Database issues**: Run `aj reset` to recreate the database schema
