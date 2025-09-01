# `aj interactive` 🗣️

Start a REPL‑style chat. `aj` uses a vectordb to store embeddings of past messages and recalls relevant prior context.

```bash
aj interactive
```

![aj interactive cli example](./aj_interactive.gif)

## 🧠 Features
Remembers salient turns via [HNSW](https://github.com/rust-cv/hnsw) + [sentence embeddings](https://github.com/guillaume-be/rust-bert).
- Limits total tokens to your configured quota (oldest context trimmed)
- Supports templates and system prompts

## 💡 Pro Tips
- `aj interactive` expects an ASCII escape code to send your message. On macOS that's `Ctrl-d`.
- Send `exit` or `Ctrl-c` to exit the REPL.

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