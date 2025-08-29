# Awful Jade (aj) 🐍🧠

[![Crates.io](https://img.shields.io/crates/v/awful_aj.svg)](https://crates.io/crates/awful_aj)
[![Docs.rs](https://docs.rs/awful_aj/badge.svg)](https://docs.rs/awful_aj)

Awful Jade (aka **`aj`**) is your command-line sidekick for working with Large Language Models (LLMs).  

Think of it as an _LLM Swiss Army knife with the best intentions_ 😇:  
Ask questions, run interactive sessions, sanitize messy OCR book dumps, synthesize exam questions…  
all without leaving your terminal.

It’s built in Rust for speed, safety, and peace of mind. 🦀

![Awful Jade CLI tool logo](aj.png)

---

## ✨ Features

- **Ask the AI**: Run `aj ask "question"` and get answers powered by your configured model.  
- **Interactive Mode**: A REPL-style conversation with memory & vector search (your AI “remembers” past context).  
- **Vector Store**: Uses HNSW + sentence embeddings to remember what you’ve said before. Basically, your AI gets a brain. 🧠  
- **Brains with Limits**: Keeps only as many tokens as you allow. When full, it forgets the oldest stuff. (Like you after 3 AM pizza.)  
- **Config & Templates**: YAML-driven configs and prompt templates. Customize everything, break nothing.  
- **Auto-downloads BERT embeddings model**: If the required `all-mini-lm-l12-v2` model isn’t around, `aj` will politely fetch and unzip it into your config dir.  

---

## 📦 Installation

From [crates.io](https://crates.io/crates/awful_aj):

```bash
cargo install awful_aj
```

This gives you the aj binary.

Requirements
	•	Rust (use rustup if you don’t have it).
	•	Diesel CLI if you want to reset or migrate the session DB.
	•	Python 3.11 and pytorch 2.4.0.
	

The model (all-mini-lm-l12-v2) will be downloaded automatically into your platform’s config directory (thanks to directories):
	•	macOS: ~/Library/Application Support/com.awful-sec.aj/
	•	Linux: ~/.config/aj/
	•	Windows: C:\Users\YOU\AppData\Roaming\awful-sec\aj/

---

## Setup (steps will vary according to your operating system)

1. Install conda python version manager.

```bash
brew install miniconda
```

2. Create Python 3.11 virtual environment named aj and activate it.

```bash
conda create -n aj python=3.11
conda activate aj
````

3. Install pytorch 2.4.0

```bash
pip install torch==2.4.0 --index-url https://download.pytorch.org/whl/cp
````

4. Add the following to your shell initialization.

```bash
export LIBTORCH_USE_PYTORCH=1
export LIBTORCH='/opt/homebrew/Caskroom/miniconda/base/pkgs/pytorch-2.4.0-py3.11_0/lib/python3.11/site-packages/torch' # Or wherever Conda installed libtorch on your OS
export DYLD_LIBRARY_PATH="$LIBTORCH/lib"

conda activate aj
```

---

## 🚀 Usage

1. Initialize

Create default configs and templates:

aj init

This will generate:
	•	config.yaml with sensible defaults
	•	templates/default.yaml and templates/simple_question.yaml
	•	A SQLite database (aj.db) for sessions

---

2. Ask a Question

aj ask "Is Bibi really from Philly?"

You’ll get a colorful, model-dependent answer.

---

3. Interactive Mode

Talk with the AI like it’s your therapist, mentor, or rubber duck:

aj interactive

Supports memory via the vector store, so it won’t immediately forget your name.
(Unlike your barista.)

---

4. Configuration

Edit your config at:

~/.config/aj/config.yaml   # Linux
~/Library/Application Support/com.awful-sec.aj/config.yaml   # macOS

Example:

api_base: "http://localhost:1234/v1"
api_key: "CHANGEME"
model: "jade_qwen3_4b_mlx"
context_max_tokens: 8192
assistant_minimum_context_tokens: 2048
stop_words:
  - "<|im_end|>\\n<|im_start|>"
  - "<|im_start|>\n"
session_db_url: "/Users/you/Library/Application Support/com.awful-sec.aj/aj.db"


---

5. Templates

Templates are YAML files in your config directory.
Here’s a baby template:

system_prompt: "You are Awful Jade, a helpful AI assistant programmed by Awful Security."
messages: []

Add more, swap them in with --template <name>.

---

🧠 How it Works
	•	Brain: Keeps memories in a deque, trims when it gets too wordy.
	•	VectorStore: Embeds your inputs using all-mini-lm-l12-v2, saves to HNSW index.
	•	Config: YAML-based, sane defaults, easy to tweak.
	•	Templates: Prompt engineering without copy-pasting into your terminal like a caveman.
	•	Ensure All Mini: If the BERT model’s not there, AJ fetches it automagically.

---

🧑‍💻 Development

Clone, hack, repeat:

git clone https://github.com/graves/awful_aj.git
cd awful_aj
cargo build

Run tests:

cargo test

---

🤝 Contributing

PRs welcome!
Bugs, docs, new templates, vector hacks—bring it on.
But remember: with great power comes great YAML.

--

📜 License

MIT. Do whatever you want, just don’t blame us when your AI remembers your browser history.

---

💡 Awful Jade: bad name, good brain.

---

