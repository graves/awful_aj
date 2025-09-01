# `aj ask` ✨

Ask a single question and print the assistant’s response.

```bash
aj ask "Is Bibi really from Philly?"
```

![aj ask cli example](./aj_ask.gif)

## 🔧 Options
- `--template` <name>: Use a specific prompt template.
- `--model` <id>: Override model for this question.
- `--session`: Session name for long running conversations.

## ✅ When to use
- Quick facts, transformations, summaries.
- Scriptable one‑liners in shell pipelines.
- Modify the default template and add a session name to give your computer a personality.

### 🙋🏻‍♀️ Help

```bash
λ aj ask --help
Ask a single question and print the assistant’s response.

If no `question` is provided, the application supplies a default prompt.

Aliases: `a`

Usage: aj ask [OPTIONS] [QUESTION]

Arguments:
  [QUESTION]
          The question to ask. When omitted, a default question is used

Options:
  -t <template>
          Name of the chat template to load (e.g., `simple_question`).

          Templates live under the app’s config directory, usually at: - macOS: `~/Library/Application Support/com.awful-sec.aj/templates/` - Linux: `~/.config/aj/templates/` - Windows: `%APPDATA%\\com.awful-sec\\aj\\templates\\`

  -s <session>
          Session name. When set, messages are persisted under this conversation.

          Using a session enables retrieval-augmented context from prior turns.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```