# Pretty Printing 🎨

The `-p/--pretty` flag transforms the assistant's plain text responses into beautifully formatted terminal output with markdown rendering and syntax highlighting.

## 🎯 What is Pretty Printing?

**Pretty printing** enhances the visual presentation of LLM responses by:
1. **Rendering markdown**: Headers, lists, bold, italic, etc.
2. **Syntax highlighting**: Language-aware code block coloring
3. **Stream-then-replace**: Shows raw output during streaming, then replaces with formatted version

This makes complex responses (especially those with code examples) significantly more readable.

## 🔧 How It Works

### Rendering Pipeline

```
┌──────────────────────┐
│ LLM Response Stream  │  Token by token
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Raw Display          │  Show streaming text in yellow
│ (during streaming)   │
└──────────┬───────────┘
           │
           ▼ (stream complete)
┌──────────────────────┐
│ Markdown Parser      │  Parse markdown structure
│                      │  Extract code blocks
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Syntax Highlighting  │  Syntect with base16-ocean.dark
│                      │  Language-specific coloring
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Terminal Rendering   │  ANSI escape codes
│ (Termimad)           │  Replace raw output
└──────────────────────┘
```

### Technologies Used

- **[Termimad](https://github.com/Canop/termimad)**: Markdown rendering in terminal
- **[Syntect](https://github.com/trishume/syntect)**: Syntax highlighting engine
- **Theme**: `base16-ocean.dark` (built into Syntect)
- **Markdown Parser**: CommonMark-compatible

## 🚀 Usage

### Basic Pretty Printing

```bash
# Regular output (plain text)
aj ask "Show me a Rust error handling example"

# Pretty-printed output (formatted)
aj ask -p "Show me a Rust error handling example"
```

**Without `-p`**:
```
Here's an example:

fn read_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

fn main() {
    match read_file("data.txt") {
        Ok(contents) => println!("{}", contents),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**With `-p`**:
```
Here's an example:

[beautifully highlighted code with colors]
fn read_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

fn main() {
    match read_file("data.txt") {
        Ok(contents) => println!("{}", contents),
        Err(e) => eprintln!("Error: {}", e),
    }
}
[end of highlighted block]
```

### Combining with RAG

```bash
# Get formatted answers from documentation
aj ask -r README.md -p "Explain the installation process"

# Beautiful code explanations
aj ask -r src/main.rs -p "How does the main function work?"
```

### Interactive Mode

```bash
# Every response is beautifully formatted
aj interactive -p

# With sessions and RAG
aj interactive -s dev -r docs/ -p
```

## 🎨 Supported Markdown Features

### Headers

**Input**:
```markdown
# Header 1
## Header 2
### Header 3
```

**Output**: Rendered with appropriate sizing and colors

### Emphasis

**Input**:
```markdown
This is **bold** and this is *italic*.
```

**Output**: Rendered with ANSI bold and italic codes

### Lists

**Input**:
```markdown
- Item 1
- Item 2
  - Nested item
  - Another nested item
- Item 3
```

**Output**: Properly indented with bullet points

### Code Blocks

**Input**:
````markdown
```rust
fn main() {
    println!("Hello, world!");
}
```
````

**Output**: Syntax-highlighted based on language tag

### Inline Code

**Input**:
```markdown
Use the `println!` macro to print.
```

**Output**: Distinct formatting for inline code

### Blockquotes

**Input**:
```markdown
> This is a quote.
> It can span multiple lines.
```

**Output**: Rendered with quote styling

### Links

**Input**:
```markdown
[Check the docs](https://docs.rs/awful_aj)
```

**Output**: Rendered with link text (URL not clickable in terminal, but visible)

## 💻 Syntax Highlighting

### Supported Languages

Syntect supports **hundreds of languages** via TextMate grammar definitions. Common examples:

| Language | Code Block Tag |
|----------|----------------|
| Rust | ` ```rust ` |
| Python | ` ```python ` or ` ```py ` |
| JavaScript | ` ```javascript ` or ` ```js ` |
| TypeScript | ` ```typescript ` or ` ```ts ` |
| C | ` ```c ` |
| C++ | ` ```cpp ` or ` ```c++ ` |
| Go | ` ```go ` |
| Java | ` ```java ` |
| Shell | ` ```bash ` or ` ```sh ` |
| JSON | ` ```json ` |
| YAML | ` ```yaml ` or ` ```yml ` |
| TOML | ` ```toml ` |
| Markdown | ` ```markdown ` or ` ```md ` |
| SQL | ` ```sql ` |
| HTML | ` ```html ` |
| CSS | ` ```css ` |

And many more! See [Syntect's language list](https://github.com/trishume/syntect#supported-languages) for the full set.

### Color Theme

`awful_aj` uses the **`base16-ocean.dark`** theme:
- **Background**: Dark blue-gray
- **Keywords**: Purple, blue
- **Strings**: Green
- **Comments**: Gray
- **Functions**: Yellow
- **Types**: Orange

This theme is optimized for readability on dark terminal backgrounds.

### Fallback Behavior

If a language is not recognized (e.g., ` ```unknown `), the code block is rendered as plain text without syntax highlighting.

## 🎭 Stream-then-Replace Behavior

Pretty printing uses a **two-phase rendering** approach:

### Phase 1: Streaming (Raw Output)

While the LLM is generating tokens:
```
⠋ Thinking...
Here's an example:

fn main() {
    println!("Hello, world!");
}

[tokens continue to stream...]
```

Output is shown in **real-time** (yellow text by default) so you get immediate feedback.

### Phase 2: Replacement (Formatted Output)

Once streaming completes:
1. **Clear** the raw output from terminal
2. **Parse** the full markdown
3. **Highlight** code blocks
4. **Render** formatted version

The screen is **replaced** with the beautifully formatted result.

### Why This Approach?

- **Immediate feedback**: You see the response as it's generated
- **Best of both worlds**: Streaming UX + final formatting
- **No flickering**: Only replaces once, at the end

### Technical Implementation

Uses ANSI escape codes:
```rust
// Clear previous lines
print!("\x1B[{}A\x1B[2K", num_lines); // Move up + clear

// Render formatted output
print_formatted_markdown(&response);
```

## ⚙️ Configuration

Pretty printing is **opt-in** via the `-p` flag. There are no additional configuration options in `config.yaml` (yet).

### Global Default

If you want pretty printing **always enabled**, create a shell alias:

```bash
# ~/.bashrc or ~/.zshrc
alias aj='aj -p'

# Or for specific commands
alias ajp='aj ask -p'
alias aji='aj interactive -p'
```

### Future Configuration

Planned options (not yet implemented):
- `default_pretty: true` in `config.yaml`
- `--theme` flag to choose syntax highlighting theme

## 💡 Best Practices

### 1. Use for Code-Heavy Responses

Pretty printing shines when the assistant generates code:
```bash
# Great use case
aj ask -p "Show me a Rust async example"

# Less impactful (plain text response)
aj ask -p "What is 2+2?"
```

### 2. Combine with Templates

Create a template that encourages code examples:

**`~/.config/aj/templates/code_helper.yaml`**:
```yaml
system_prompt: |
  You are a coding assistant. Always provide:
  1. Brief explanation
  2. Code example with syntax highlighting
  3. Usage notes

  Use markdown formatting and language-specific code blocks.

messages: []
```

**Usage**:
```bash
aj ask -t code_helper -p "How do I read a file in Python?"
```

### 3. Interactive Debugging

Pretty mode is excellent for REPL-style coding sessions:
```bash
aj interactive -p

You: "Show me error handling in Go"
[Beautifully formatted code with syntax highlighting]

You: "Now show panic recovery"
[More highlighted examples]
```

### 4. RAG + Pretty for Documentation

When querying technical docs, pretty printing makes output much more readable:
```bash
aj ask -r docs/api.md -p "Show me the authentication example"
```

### 5. Disable for Scripting

If piping output to another program, **disable** pretty printing:
```bash
# Bad (includes ANSI codes)
aj ask -p "What is 2+2?" | grep 4

# Good (plain text)
aj ask "What is 2+2?" | grep 4
```

## 🐛 Troubleshooting

### Colors Don't Appear

**Cause**: Terminal doesn't support ANSI colors.

**Fix**:
- Use a modern terminal (iTerm2, Alacritty, Windows Terminal)
- Check `$TERM` environment variable: `echo $TERM`
  - Should be `xterm-256color` or similar

### Code Block Not Highlighted

**Cause**: Language not recognized or misspelled.

**Fix**: Check the language tag:
```markdown
<!-- Wrong -->
```rust-lang
fn main() {}
```

<!-- Correct -->
```rust
fn main() {}
```

### Rendering Issues (Garbled Output)

**Cause**: Terminal width issues or unsupported characters.

**Fix**:
- Resize terminal to at least 80 columns
- Ensure UTF-8 encoding: `export LC_ALL=en_US.UTF-8`

### Stream Replacement Doesn't Work

**Cause**: Some terminals don't support ANSI cursor movement.

**Workaround**: Streaming will still work, but you'll see both raw + formatted output (not replaced).

### Performance Lag

**Cause**: Very large responses (>10,000 tokens) take time to parse and highlight.

**Workaround**: Use `-p` only for reasonably-sized responses, or disable for very long outputs.


## 🔗 Related Features

- **[RAG](./rag.md)**: Combine pretty printing with document retrieval for beautiful documentation answers
- **[Templates](./templates/README.md)**: Create templates that leverage markdown formatting
- **[Interactive Mode](./use/interactive.md)**: Pretty printing enhances the REPL experience

## 🎉 Conclusion

Pretty printing transforms `awful_aj` from a functional tool into a delightful coding companion. By combining markdown rendering and syntax highlighting, complex technical responses become significantly more readable and actionable.

Enable it with `-p` and enjoy beautiful terminal output!
