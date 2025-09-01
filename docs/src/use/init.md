# `aj init` 🏗️

Create default config, templates, and the session database.

```bash
aj init
```

## 📁 What it creates
- `config.yaml` with sensible defaults
- `templates/default.yaml`, `templates/simple_question.yaml`
- A SQLite database `aj.db` for sessions

## 📍 Where these live
- macOS: `~/Library/Application Support/com.awful-sec.aj/`
- Linux: `~/.config/aj/`
- Windows: `C:\\Users\\YOU\\AppData\\Roaming\\awful-sec\\aj\\`

## 🙋🏻‍♀️ Help
```bash
aj init --help
Initialize configuration and default templates in the platform config directory.

Creates the config file and a minimal template set if they don’t exist yet.

Usage: aj init

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```