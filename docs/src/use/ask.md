# `aj ask` ✨

Ask a single question and print the assistant’s response.

```bash
aj ask "Is Bibi really from Philly?"
```

![aj ask cli example](./aj_ask.gif)

## 🔧 Options
- `-t, --template` <name>: Use a specific prompt template.
- `-s, --session` <name>: Session name for long running conversations.
- `--one-shot`: Force one-shot mode, ignoring any session configured in config.yaml.

## 🎨 Output Colors
- Assistant responses appear in **yellow**
- Model reasoning (in `<think>` tags) appears in **dark gray**

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

      --one-shot
          Force one-shot mode, ignoring any session configured in config.yaml.

          When this flag is set, the prompt will be treated as standalone
          with no memory or session tracking, even if a session_name is
          configured in the config file.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
