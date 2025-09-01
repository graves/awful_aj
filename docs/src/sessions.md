# Sessions 🗂️

`aj` stores conversations in a local SQLite database so you can review, continue, or mine for insights. Sessions also capture config snapshots so you know which model/settings produced which answers. 🧠💾

## 📍 Where is it?
- Typically alongside your config under `aj.db`.
- Change via `session_db_url` in your [Config file](./config.md).

## 🧰 What’s inside?
- Message turns (user / assistant)
- Template and model metadata

## 🧽 Maintenance
- Backup: copy the `.db` file while `aj` is not running.
- Reset/Migrate: if you use Diesel CLI, run your migrations (optional, advanced).
```shell
 diesel database reset --database-url "/Users/you/Library/Application Support/com.awful-sec.aj/aj.db"
 ```

## 📍 Where is it?

By default, next to your platform config directory as aj.db. Change this via session_db_url in your Config. Examples:
	•	macOS: `~/Library/Application Support/com.awful-sec.aj/aj.db`
	•	Linux: `~/.config/aj/aj.db`
	•	Windows: `C:\Users\YOU\AppData\Roaming\awful-sec\aj\aj.db`

> Tip: Use absolute paths; Diesel’s DATABASE_URL and the app’s session_db_url should point to the same file when you run migrations.

## 🧰 What’s inside?

Three tables, exactly as modeled in your code:
- `conversations` – one row per session (session_name unique-ish namespace)
- `messages` – one row per turn (system/user/assistant), FK → conversation
- `awful_configs` – point-in-time snapshots of runtime settings, FK → conversation

Rust models (for reference)
- `Conversation { id, session_name }`
- `Message { id, role, content, dynamic, conversation_id }`
- `AwfulConfig { id, api_base, api_key, model, context_max_tokens, assistant_minimum_context_tokens, stop_words, conversation_id }`


## 🧪 The Diesel schema (generated)

The Diesel `schema.rs` corresponds to:
```rust
// @generated automatically by Diesel CLI.

diesel::table! {
    awful_configs (id) {
        id -> Integer,
        api_base -> Text,
        api_key -> Text,
        model -> Text,
        context_max_tokens -> Integer,
        assistant_minimum_context_tokens -> Integer,
        stop_words -> Text,
        conversation_id -> Nullable<Integer>,
    }
}

diesel::table! {
    conversations (id) {
        id -> Integer,
        session_name -> Text,
    }
}

diesel::table! {
    messages (id) {
        id -> Integer,
        role -> Text,
        content -> Text,
        dynamic -> Bool,
        conversation_id -> Nullable<Integer>,
    }
}

diesel::joinable!(awful_configs -> conversations (conversation_id));
diesel::joinable!(messages -> conversations (conversation_id));

diesel::allow_tables_to_appear_in_same_query!(
    awful_configs,
    conversations,
    messages,
);
```

### Option A — Use Diesel CLI 🛠️

This is the most ergonomic way to create and evolve the DB.

1. Get the schema

```shell
git clone https://github.com/graves/awful_aj
cd awful_aj
```

2. Install Diesel CLI (SQLite only)

#### macOS / Linux
```
cargo install diesel_cli --no-default-features --features sqlite
```
#### Windows (PowerShell)
```
cargo install diesel_cli --no-default-features --features sqlite
```
On macOS you may need system SQLite headers: `brew install sqlite` (and ensure `pkg-config` can find it).

3. Set your database URL

#### macOS
```sh
export DATABASE_URL="$HOME/Library/Application Support/com.awful-sec.aj/aj.db"
```
#### Linux
```sh
export DATABASE_URL="$HOME/.config/aj/aj.db"
```
#### Windows (PowerShell)
```powershell
$env:DATABASE_URL = "$env:APPDATA\awful-sec\aj\aj.db"
```

4. Run migrations

```shell
diesel migration run
diesel print-schema > src/schema.rs   # keep your schema.rs in sync
```

5.  Reset / Recreate (when needed)

`diesel database reset`   # drops and recreates (uses up/down)

> 🧠 Gotcha: Always point `DATABASE_URL` to the same file your app will use (`session_db_url`). If you migrate one file and run the app on another path, you’ll see "missing table" errors.

### Option B — No CLI: Embedded Migrations (pure Rust) 🧰

If you don’t want to depend on the CLI, bundle SQL with your binary and run it on startup using diesel_migrations.

1. Add the crate
```rust
# Cargo.toml
[dependencies]
diesel = { version = "2", features = ["sqlite"] }
diesel_migrations = "2"
```

2. Create an in-repo migrations folder
```shell
src/
migrations/
  00000000000000_init_aj_schema/
    up.sql
    down.sql
```

Use the same SQL as in Option A’s `up.sql`/`down.sql`.

3) Run at startup
```rust
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, MigrationHarness};

// Embed migrations from the `migrations/` dir
const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!("./migrations");

fn establish_connection(database_url: &str) -> SqliteConnection {
    SqliteConnection::establish(database_url)
        .unwrap_or_else(|e| panic!("Error connecting to {database_url}: {e}"))
}

pub fn run_migrations(database_url: &str) {
    let mut conn = establish_connection(database_url);
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Migrations failed");
}
```

Call `run_migrations(&cfg.session_db_url)` once during app startup. ✅

Bonus: You can ship a single binary that self-provisions its SQLite schema on first run—no CLI needed.

### Option C — No Diesel at all: Raw sqlite3 🪚

For ultra-minimal environments, create the file and tables directly.

macOS / Linux
```shell
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

Windows (PowerShell):
```rust
$DB = "$env:APPDATA\awful-sec\aj\aj.db"
sqlite3 $DB @"
PRAGMA foreign_keys = ON;
-- (same CREATE TABLE statements as above)
"@
```
You can now use Diesel from your app against that file.

## 🔍 Verifying & Inspecting

List tables
```shell
sqlite3 "$DATABASE_URL" ".tables"
```
Peek at last 10 messages
```shell
sqlite3 "$DATABASE_URL" "SELECT id, role, substr(content,1,60) || '…' as snippet FROM messages ORDER BY id DESC LIMIT 10;"
```
Check a conversation by name
```shell
SELECT * FROM conversations WHERE session_name = 'default';
```

## 🧽 Maintenance
- Backup: copy the .db file while `aj` is not running.
- Vacuum (reclaim space):
```shell
sqlite3 "$DATABASE_URL" "VACUUM;"
```
- Integrity check:
```shell
sqlite3 "$DATABASE_URL" "PRAGMA integrity_check;"
```
- Reset via Diesel:
```
diesel database reset
```

> Tip: Enable foreign keys at connection open (PRAGMA foreign_keys = ON;). Diesel’s SQLite backend does not enforce this automatically unless you set the pragma on each connection (or in migrations as above).

## 🧯 Troubleshooting
- "no such table: conversations"
-    You migrated a different file than you’re connecting to. Recheck `DATABASE_URL` vs `session_db_url`.
- Diesel CLI build fails on macOS
    - Install headers: brew install sqlite and ensure pkg-config is available.
- Foreign keys not enforced
    - Ensure `PRAGMA foreign_keys = ON;` is set (included in `up.sql`). For safety, set it again immediately after opening each connection.
- Schema drift
    - If you edit SQL manually, regenerate schema.rs with: `diesel print-schema > src/schema.rs`

## 🧪 Example: Insert a Conversation + Message (Diesel)
```rust
use diesel::prelude::*;
use awful_aj::schema::{conversations, messages};
use awful_aj::models::{Conversation, Message};

fn demo(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let convo: Conversation = diesel::insert_into(conversations::table)
        .values(&Conversation { id: None, session_name: "demo".into() })
        .returning(Conversation::as_returning())
        .get_result(conn)?;

    let _msg: Message = diesel::insert_into(messages::table)
        .values(&Message {
            id: None,
            role: "user".into(),
            content: "Hi".into(),
            dynamic: false,
            conversation_id: convo.id,
        })
        .returning(Message::as_returning())
        .get_result(conn)?;

    Ok(())
}
```

> All set! Whether you prefer Diesel CLI, embedded migrations, or plain `sqlite3`, you’ve got everything needed to provision, migrate, and operate the aj session database cleanly. 🧰✨