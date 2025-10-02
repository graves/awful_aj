# Install on Linux 🐧

## ✅ Requirements
- [Rust](https://rustup.rs/)
- [Miniconda](https://docs.conda.io/en/latest/miniconda.html) (recommended) 🐍
- Python 3.11
- PyTorch 2.4.0

### 1. Python via conda 🧪
```shell
# Install Miniconda (example for Debian/Ubuntu)
sudo apt-get update
sudo apt-get install -y wget bzip2

wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
bash Miniconda3-latest-Linux-x86_64.sh

# Create and activate environment
conda create -n aj python=3.11 -y
conda activate aj
```

### 2. Install PyTorch 2.4.0 🧱
```shell
pip install torch==2.4.0
```

### 3. Environment setup 🌿

Add to your shell init (e.g., `~/.zshrc`):

```shell
export LIBTORCH_USE_PYTORCH=1
export LIBTORCH="$HOME/miniconda3/envs/aj/lib/python3.11/site-packages/torch"
export LD_LIBRARY_PATH="$LIBTORCH/lib:$LD_LIBRARY_PATH"
```

### 4. Install from crates.io and initialize 📦
```shell
cargo install awful_aj
cargo init
```

`cargo init` creates:
- `~/.config/aj/`
- `~/.config/aj/config.yaml`
- `~/.config/aj/templates/`
- `~/.config/aj/templates/default.yaml`
- `~/.config/aj/templates/simple_question.yaml`

### 5. Prepare the Session Database (SQLite) 📂

`aj` stores sessions, messages, and configs in a local SQLite3 database (`aj.db`).
You have two ways to provision it:

#### Option A — Without Diesel CLI (raw sqlite3)

This is the minimal approach if you don’t want extra tooling.

```shell
sqlite3 ~/.config/aj/aj.db <<'SQL'
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS conversations (
  id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  session_name  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
  id               INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  role             TEXT NOT NULL,
  content          TEXT NOT NULL,
  dynamic          BOOLEAN NOT NULL DEFAULT 0,
  conversation_id  INTEGER,
  FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS awful_configs (
  id                                 INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  api_base                            TEXT NOT NULL,
  api_key                             TEXT NOT NULL,
  model                               TEXT NOT NULL,
  context_max_tokens                  INTEGER NOT NULL,
  assistant_minimum_context_tokens    INTEGER NOT NULL,
  stop_words                          TEXT NOT NULL,
  conversation_id                     INTEGER,
  FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
SQL
```

Verify tables:
```shell
sqlite3 ~/.config/aj/aj.db ".tables"
```

#### Option B — With Diesel CLI 🛠️

This is recommended if you want migrations and a typed schema.rs.

Grab the `awful_aj` git repo.
```shell
git clone https://github.com/graves/awful_aj
cd awful_aj
```

Install Diesel CLI for SQLite.

```shell
cargo install diesel_cli --no-default-features --features sqlite
```

Configure database URL and run migrations.

```shell
export DATABASE_URL="$HOME/.config/aj/aj.db"
diesel migration run
```

### 6. First‑run model download ⤵️

On first use needing embeddings, `aj` downloads `all-mini-lm-l12-v2` from https://awfulsec.com/bigfiles/all-mini-lm-l12-v2.zip into:

`~/.config/aj/`

You’re ready! ✅

Try:
```shell
aj ask "Hello from macOS!"
```