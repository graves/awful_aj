# Install on macOS 🍎

## ✅ Requirements
- [Miniconda](https://docs.conda.io/en/latest/miniconda.html) (recommended) 🐍
- Python 3.11
- PyTorch 2.4.0

### 1. Python via conda 🧪
```shell
brew install miniconda               # or use the official installer
conda create -n aj python=3.11 -y
conda activate aj
```

### 2. Install PyTorch 2.4.0 🧱
```shell
pip install torch==2.4.0 --index-url https://download.pytorch.org/whl/cp
```

### 3. Environment setup 🌿

Add to your shell init (e.g., `~/.zshrc`):

```shell
export LIBTORCH_USE_PYTORCH=1
export LIBTORCH="/opt/homebrew/Caskroom/miniconda/base/pkgs/pytorch-2.4.0-py3.11_0/lib/python3.11/site-packages/torch"
export DYLD_LIBRARY_PATH="$LIBTORCH/lib"
```

### 4. Install from crates.io and initialize 📦
```shell
cargo install awful_aj
cargo init
```

`cargo init` creates:
- `~/Library/Application Support/com.awful-sec.aj/`
- `~/Library/Application Support/com.awful-sec.aj/config.yaml`
- `~/Library/Application Support/com.awful-sec.aj/templates`
- `~/Library/Application Support/com.awful-sec.aj/templates/default.yaml`
- `~/Library/Application Support/com.awful-sec.aj/templates/simple_question.yaml`

### 5. Prepare the Session Database (SQLite) 📂

`aj` stores sessions, messages, and configs in a local SQLite3 database (`aj.db`).
You have two ways to provision it:

#### Option A — Without Diesel CLI (raw sqlite3)

This is the minimal approach if you don’t want extra tooling.

```shell
# Create the DB file
sqlite3 "$HOME/Library/Application Support/com.awful-sec.aj/aj.db" <<'SQL'
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
```
sqlite3 "$HOME/Library/Application Support/com.awful-sec.aj/aj.db" ".tables"
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
export DATABASE_URL="$HOME/Library/Application Support/com.awful-sec.aj/aj.db"
diesel migration run
```

### 6. First‑run model download ⤵️

On first use needing embeddings, `aj` downloads `all-mini-lm-l12-v2` from https://awfulsec.com/bigfiles/all-mini-lm-l12-v2.zip into:

`~/Library/Application Support/com.awful-sec.aj/`

You’re ready! ✅

Try:

```shell
aj ask "Hello from macOS!"
```