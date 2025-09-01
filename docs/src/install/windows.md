# Install on Windows 🪟

## ✅ Requirements
- Miniconda (recommended) 🐍
- Python 3.11
- PyTorch 2.4.0
- SQLite3 (or use Diesel CLI for migrations)

### 1. Python via conda 🧪

Open PowerShell (with Conda available on `PATH`):
```shell
winget install miniconda3                 # or use the official installer
conda create -n aj python=3.11 -y
conda activate aj
```

### 2. Install PyTorch 2.4.0 🧱
```shell
pip install torch==2.4.0 --index-url https://download.pytorch.org/whl/cp
```

### 3. Environment setup 🌿

Add these to your shell init PowerShell profile:
`$PROFILE` → e.g. `C:\Users\YOU\Documents\PowerShell\Microsoft.PowerShell_profile.ps1`:
```powershell
$env:LIBTORCH_USE_PYTORCH = "1"
$env:LIBTORCH = "C:\Users\YOU\miniconda3\envs\aj\Lib\site-packages\torch"
$env:PATH = "$env:LIBTORCH\lib;$env:PATH"
```
Reload your profile or open a new shell.

### 4. Install from crates.io and initialize 📦
```shell
cargo install awful_aj
aj init
```

`aj init` creates:
- `C:\Users\YOU\AppData\Roaming\awful-sec\aj\`
- `config.yaml`
- `templates\`
- `templates\default.yaml`
- `templates\simple_question.yaml`

### 5. Prepare the Session Database (SQLite) 📂

`aj` stores sessions, messages, and configs in a local SQLite3 database (`aj.db`).
You have two ways to provision it:

#### Option A — Without Diesel CLI (raw sqlite3)

Minimal setup if you don’t want extra tooling. Ensure you have `sqlite3.exe` in `PATH`.
```shell
$DB="$env:APPDATA\awful-sec\aj\aj.db"

sqlite3 $DB @"
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
"@

Verify tables:

sqlite3 $DB ".tables"
```

#### Option B — With Diesel CLI 🛠️

Recommended if you want migrations and a typed schema.rs.
1. Grab the repo:
```shell
git clone https://github.com/graves/awful_aj
cd awful_aj
```
2. Install Diesel CLI for SQLite:
```shell
cargo install diesel_cli --no-default-features --features sqlite
```
3. Configure database URL and run migrations:
```shell
$env:DATABASE_URL="$env:APPDATA\awful-sec\aj\aj.db"
diesel migration run
```

### 6. First-run model download ⤵️

On first use needing embeddings, `aj` downloads all-mini-lm-l12-v2 from
https://awfulsec.com/bigfiles/all-mini-lm-l12-v2.zip into:

`C:\Users\YOU\AppData\Roaming\awful-sec\aj\`

## ✅ Quick sanity check
```shell
aj ask "Hello from Windows!"
```