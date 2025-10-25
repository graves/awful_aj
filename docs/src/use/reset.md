# `aj reset` 🔄

Reset the database to a pristine state.

```bash
aj reset
```

## 🧹 What it does
- Drops all sessions, messages, and configurations from the database
- Recreates the schema from scratch
- Gives you a clean slate for new sessions

## ⚠️ Warning
This operation is **destructive**. All conversation history, sessions, and stored messages will be permanently deleted.

## ✅ When to use
- You want to start fresh without old conversation history
- Database has become corrupted or problematic
- Testing or development purposes
- Cleaning up after experiments

## 🙋🏻‍♀️ Help

```bash
λ aj reset --help
Reset the database to a pristine state.

This command drops all sessions, messages, and configurations from the database,
then recreates the schema. Use this to start fresh with a clean database.

Aliases: `r`

Usage: aj reset

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## 📝 Example

```bash
# Reset the database
aj reset

# Or use the alias
aj r
