# `aj interactive` 🗣️

Start a REPL‑style chat. `aj` uses a vectordb to store embeddings of past messages and recalls relevant prior context.

```bash
aj interactive
```

![aj interactive cli example](./aj_interactive.gif)

## 🧠 Features
Remembers salient turns via [HNSW](https://github.com/rust-cv/hnsw) + sentence embeddings (using [Candle](https://github.com/huggingface/candle) for pure Rust ML).
- Limits total tokens to your configured quota (oldest context trimmed)
- Supports templates and system prompts

## 🎨 Output Colors
- Your input appears in **blue**
- Assistant responses appear in **yellow**
- Model reasoning (in `<think>` tags) appears in **dark gray**

## 🔧 Options
- `-t, --template` <name>: Use a specific prompt template.
- `-s, --session` <name>: Session name for the conversation.
- `-r, --rag` <files>: Comma-separated list of plain text files for RAG context (loaded once at startup).
- `-k, --rag-top-k` <number>: Maximum number of RAG chunks to inject per query (default: 3).
- `-p, --pretty`: Enable pretty-printing with markdown rendering and syntax highlighting.

## 💡 Pro Tips
- `aj interactive` expects an ASCII escape code to send your message. On macOS that's `Ctrl-d`.
- Send `exit` or `Ctrl-c` to exit the REPL.

## 📚 Examples

### Interactive session with RAG
```bash
# Load documentation for all questions in the session
aj interactive -r "docs/api.md,docs/guide.md"

# Combine with session persistence
aj interactive -s my-project -r project-notes.txt
```

### Pretty-printed REPL
```bash
# Get beautiful formatted responses
aj interactive -p

# Full-featured development assistant
aj interactive -s dev -r "README.md,CHANGELOG.md" -p
```

## 🙋🏻‍♀️ Help
```bash
λ aj interactive --help
Start an interactive REPL-style conversation.

Prints streaming assistant output (when enabled) and persists messages if a session name is configured by the application.

Aliases: `i`

Usage: aj interactive [OPTIONS]

Options:
  -t <template>
          Name of the chat template to load (e.g., `simple_question`)

  -s <session>
          Session name for the conversation

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
