//! # Pretty Printing - Markdown Rendering and Syntax Highlighting
//!
//! This module provides beautiful terminal output for markdown-formatted text with syntax-
//! highlighted code blocks. It's used to display LLM responses, documentation, and help text
//! in an attractive, readable format.
//!
//! ## Overview
//!
//! The pretty-printing system consists of two main components:
//!
//! 1. **[`print_pretty()`]**: One-shot markdown rendering for complete text
//! 2. **[`PrettyPrinter`]**: Streaming markdown renderer for real-time token output
//!
//! Both use the same underlying rendering pipeline:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                   Pretty Printer                         │
//! │  ┌────────────────────┐  ┌────────────────────┐         │
//! │  │  Markdown Parser   │  │  Code Highlighter  │         │
//! │  │  (Headers, bold,   │  │  (Syntect + Theme) │         │
//! │  │   italic, inline)  │  │                    │         │
//! │  └─────────┬──────────┘  └─────────┬──────────┘         │
//! │            │                       │                     │
//! │            ▼                       ▼                     │
//! │  ┌──────────────────────────────────────────┐           │
//! │  │   Terminal Output (ANSI Escape Codes)    │           │
//! │  │   - Crossterm for color/attributes       │           │
//! │  │   - 24-bit true color for code blocks    │           │
//! │  └──────────────────────────────────────────┘           │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Supported Markdown Features
//!
//! | Markdown Syntax | Terminal Rendering | Notes |
//! |----------------|-------------------|-------|
//! | `# Header` | **Bold Cyan** | Three levels: `#`, `##`, `###` |
//! | `**bold**` | **Bold** | Text attribute |
//! | `*italic*` | *Italic* | Text attribute |
//! | `` `code` `` | Yellow monospace | Inline code |
//! | ` ```lang\ncode\n``` ` | Syntax highlighted | 40+ languages supported |
//!
//! ## Syntax Highlighting
//!
//! Code blocks are highlighted using the **Syntect** library with the `base16-ocean.dark` theme:
//!
//! - **40+ languages** supported (Rust, Python, JavaScript, etc.)
//! - **24-bit true color** for vibrant, accurate highlighting
//! - **Language detection** via multiple methods:
//!   - Language token (e.g., `rust`, `python`)
//!   - File extension (e.g., `rs`, `py`)
//!   - Common aliases (e.g., `js` → JavaScript, `sh` → Shell)
//!
//! ### Supported Language Aliases
//!
//! | Alias | Language | Extensions |
//! |-------|----------|-----------|
//! | `py` | Python | `python`, `py` |
//! | `js`, `javascript` | JavaScript | `js` |
//! | `ts`, `typescript` | TypeScript | `ts` |
//! | `rs` | Rust | `rust`, `rs` |
//! | `sh`, `bash`, `shell` | Shell | `sh` |
//! | `yml` | YAML | `yaml`, `yml` |
//! | `md` | Markdown | `markdown`, `md` |
//!
//! ## Usage Patterns
//!
//! ### One-Shot Rendering
//!
//! Use [`print_pretty()`] for complete markdown text:
//!
//! ```no_run
//! use awful_aj::pretty::print_pretty;
//!
//! let markdown = "# Vector Databases\n\
//! \n\
//! A **vector database** stores embeddings for semantic search.\n\
//! \n\
//! Use `search()` to find similar vectors.\n";
//!
//! print_pretty(markdown).unwrap();
//! ```
//!
//! **Output**:
//! ```text
//! Vector Databases          (bold cyan)
//!
//! A vector database stores embeddings for semantic search.
//!                (bold)
//!
//! Example Usage             (bold cyan)
//!
//! [rust]                    (italic gray)
//! let embeddings = model.encode(&["hello", "world"]);  (syntax highlighted)
//! index.add(embeddings);
//!
//! Use search() to find similar vectors.
//!     (yellow)
//! ```
//!
//! ### Streaming Rendering
//!
//! Use [`PrettyPrinter`] for real-time token-by-token output (ideal for LLM streaming):
//!
//! ```no_run
//! use awful_aj::pretty::PrettyPrinter;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut printer = PrettyPrinter::new();
//!
//! // Simulate streaming tokens
//! printer.add_chunk("# Hello\n\n")?;
//! printer.add_chunk("This is **bold** text.\n\n")?;
//! printer.add_chunk("```rust\n")?;
//! printer.add_chunk("fn main() {\n")?;
//! printer.add_chunk("    println!(\"Hi\");\n")?;
//! printer.add_chunk("}\n")?;
//! printer.add_chunk("```\n")?;
//!
//! // Flush any remaining buffered content
//! printer.flush()?;
//! # Ok(())
//! # }
//! ```
//!
//! **How it works**:
//! - Accumulates chunks in a buffer
//! - Detects code block boundaries (` ``` `)
//! - Prints complete lines/blocks as they form
//! - Handles partial input gracefully
//!
//! ## Implementation Details
//!
//! ### Color Rendering
//!
//! The module uses **Crossterm** for portable ANSI terminal control:
//!
//! - `SetForegroundColor(Color::Cyan)` - Header color
//! - `SetAttribute(Attribute::Bold)` - Bold text
//! - `SetAttribute(Attribute::Italic)` - Italic text
//! - `SetAttribute(Attribute::Reset)` - Clear formatting
//!
//! ### Code Block Rendering
//!
//! Code blocks use **Syntect** for syntax highlighting:
//!
//! 1. Parse language identifier from ` ```lang `
//! 2. Load syntax definition from `SyntaxSet`
//! 3. Apply `base16-ocean.dark` theme
//! 4. Highlight each line with 24-bit color codes
//! 5. Emit ANSI escape sequences for terminal
//!
//! ### Inline Formatting
//!
//! Inline markdown (bold, italic, code) is processed using regex:
//!
//! - **Inline code**: `` `text` `` → `\x1b[33m` (yellow)
//! - **Bold**: `**text**` → `\x1b[1m` (bold attribute)
//! - **Italic**: `*text*` → `\x1b[3m` (italic attribute)
//!
//! Replacements are applied sequentially, with care to avoid breaking nested patterns.
//!
//! ## Examples
//!
//! ### Rendering LLM Responses
//!
//! ```no_run
//! use awful_aj::pretty::print_pretty;
//!
//! # fn example(assistant_response: &str) -> Result<(), Box<dyn std::error::Error>> {
//! // Assume we got this from the LLM API
//! let response = "## Analysis Results\n\
//! \n\
//! The code has **2 issues**:\n\
//! \n\
//! 1. Missing error handling\n\
//! 2. Inefficient algorithm\n";
//!
//! print_pretty(response)?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Streaming LLM Output
//!
//! ```no_run
//! use awful_aj::pretty::PrettyPrinter;
//! use futures::StreamExt;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut token_stream = futures::stream::iter(vec!["#", " ", "Title", "\n", "\n", "Text"]);
//! let mut printer = PrettyPrinter::new();
//!
//! // Process streaming tokens
//! while let Some(token) = token_stream.next().await {
//!     printer.add_chunk(token)?;
//! }
//!
//! // Ensure all content is displayed
//! printer.flush()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Custom Markdown Content
//!
//! ```no_run
//! use awful_aj::pretty::print_pretty;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let help_text = "# Awful Jade Help\n\
//! \n\
//! ## Commands\n\
//! \n\
//! - `aj ask \"<question>\"` - Ask a question\n\
//! - `aj interactive` - Start interactive session\n\
//! \n\
//! Use `--help` for more options.\n";
//!
//! print_pretty(help_text)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Performance Characteristics
//!
//! | Operation | Complexity | Notes |
//! |-----------|-----------|-------|
//! | Regex matching | O(n) | Linear scan for markdown patterns |
//! | Syntax highlighting | O(n) | Per-line tokenization with Syntect |
//! | Color rendering | O(1) per token | ANSI escape code emission |
//! | Streaming buffer | O(1) amortized | Incremental line processing |
//!
//! ## Error Handling
//!
//! All public functions return `Result<(), Box<dyn Error>>` for IO errors:
//!
//! - **Terminal write failures**: Propagated to caller
//! - **Regex compilation errors**: Treated as internal errors (should never fail)
//! - **Syntax loading errors**: Fall back to plain text
//!
//! ## See Also
//!
//! - [`crate::api`] - API client that uses pretty printing for streaming responses
//! - [`crate::commands`] - CLI commands that control pretty printing via `--pretty` flag
//! - [Syntect Documentation](https://docs.rs/syntect/) - Syntax highlighting library
//! - [Crossterm Documentation](https://docs.rs/crossterm/) - Terminal manipulation library

use crossterm::{
    ExecutableCommand,
    style::{Attribute, Color, SetAttribute, SetForegroundColor},
};
use regex::Regex;
use std::error::Error;
use std::io::{stdout, Write};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

/// Print markdown text with pretty formatting and syntax-highlighted code blocks.
///
/// # Features
/// - Headers (#, ##, ###) in bold cyan
/// - Bold text (**text**) in bold
/// - Italic text (*text*) in italic
/// - Inline code (`code`) in yellow
/// - Code blocks (```lang) with syntax highlighting
/// - Lists (-, *, 1.) properly formatted
///
/// # Parameters
/// - `text`: The markdown text to render
///
/// # Errors
/// Returns IO errors if terminal output fails
pub fn print_pretty(text: &str) -> Result<(), Box<dyn Error>> {
    let mut out = stdout();

    // Load syntax highlighting assets
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

    // Split by code blocks first
    let code_block_re = Regex::new(r"```(\w+)?\n([\s\S]*?)```")?;

    let mut last_end = 0;

    for cap in code_block_re.captures_iter(text) {
        let match_start = cap.get(0).unwrap().start();
        let match_end = cap.get(0).unwrap().end();

        // Print text before code block with markdown formatting
        if match_start > last_end {
            print_markdown(&text[last_end..match_start], &mut out)?;
        }

        // Print code block with syntax highlighting
        let language = cap.get(1).map(|m| m.as_str()).unwrap_or("text");
        let code = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        print_code_block(code, language, &ps, theme, &mut out)?;

        last_end = match_end;
    }

    // Print remaining text
    if last_end < text.len() {
        print_markdown(&text[last_end..], &mut out)?;
    }

    out.flush()?;
    Ok(())
}

/// Print regular markdown text with formatting
fn print_markdown(text: &str, out: &mut std::io::Stdout) -> Result<(), Box<dyn Error>> {
    for line in text.lines() {
        // Headers
        if line.starts_with("### ") {
            out.execute(SetForegroundColor(Color::Cyan))?;
            out.execute(SetAttribute(Attribute::Bold))?;
            writeln!(out, "{}", &line[4..])?;
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(SetForegroundColor(Color::Reset))?;
        } else if line.starts_with("## ") {
            out.execute(SetForegroundColor(Color::Cyan))?;
            out.execute(SetAttribute(Attribute::Bold))?;
            writeln!(out, "{}", &line[3..])?;
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(SetForegroundColor(Color::Reset))?;
        } else if line.starts_with("# ") {
            out.execute(SetForegroundColor(Color::Cyan))?;
            out.execute(SetAttribute(Attribute::Bold))?;
            writeln!(out, "{}", &line[2..])?;
            out.execute(SetAttribute(Attribute::Reset))?;
            out.execute(SetForegroundColor(Color::Reset))?;
        } else if line.is_empty() {
            // Preserve blank lines but don't double-space them
            writeln!(out)?;
        } else {
            // Process inline formatting
            print_inline_markdown(line, out)?;
            writeln!(out)?;
        }
    }

    Ok(())
}

/// Print a line with inline markdown formatting (bold, italic, inline code)
fn print_inline_markdown(line: &str, out: &mut std::io::Stdout) -> Result<(), Box<dyn Error>> {
    // Simple regex-based inline formatting
    let inline_code_re = Regex::new(r"`([^`]+)`").unwrap();
    let bold_re = Regex::new(r"\*\*([^\*]+)\*\*").unwrap();
    let italic_re = Regex::new(r"\*([^\*]+)\*").unwrap();

    let mut processed = line.to_string();
    let mut replacements = Vec::new();

    // Find inline code spans
    for cap in inline_code_re.captures_iter(line) {
        let full_match = cap.get(0).unwrap().as_str();
        let code_text = cap.get(1).unwrap().as_str();
        replacements.push((full_match.to_string(), format!("\x1b[33m{}\x1b[0m", code_text)));
    }

    // Find bold spans
    for cap in bold_re.captures_iter(line) {
        let full_match = cap.get(0).unwrap().as_str();
        let bold_text = cap.get(1).unwrap().as_str();
        replacements.push((full_match.to_string(), format!("\x1b[1m{}\x1b[0m", bold_text)));
    }

    // Find italic spans (but not inside bold)
    for cap in italic_re.captures_iter(line) {
        let full_match = cap.get(0).unwrap().as_str();
        // Skip if this is part of a bold span
        if !full_match.starts_with("**") {
            let italic_text = cap.get(1).unwrap().as_str();
            replacements.push((full_match.to_string(), format!("\x1b[3m{}\x1b[0m", italic_text)));
        }
    }

    // Apply replacements (in reverse order to maintain positions)
    for (find, replace) in replacements {
        processed = processed.replace(&find, &replace);
    }

    write!(out, "{}", processed)?;
    Ok(())
}

/// Print a code block with syntax highlighting
fn print_code_block(
    code: &str,
    language: &str,
    ps: &SyntaxSet,
    theme: &syntect::highlighting::Theme,
    out: &mut std::io::Stdout,
) -> Result<(), Box<dyn Error>> {
    // Print code block header (language label)
    if !language.is_empty() {
        out.execute(SetForegroundColor(Color::DarkGrey))?;
        out.execute(SetAttribute(Attribute::Italic))?;
        writeln!(out, "[{}]", language)?;
        out.execute(SetAttribute(Attribute::Reset))?;
        out.execute(SetForegroundColor(Color::Reset))?;
    }

    // Get syntax for the language - try multiple methods for better detection
    let syntax = ps
        .find_syntax_by_token(language)
        .or_else(|| ps.find_syntax_by_extension(language))
        .or_else(|| {
            // Try common aliases
            match language.to_lowercase().as_str() {
                "py" => ps.find_syntax_by_extension("python"),
                "js" | "javascript" => ps.find_syntax_by_extension("js"),
                "ts" | "typescript" => ps.find_syntax_by_extension("ts"),
                "rs" => ps.find_syntax_by_extension("rust"),
                "sh" | "bash" | "shell" => ps.find_syntax_by_extension("sh"),
                "yml" => ps.find_syntax_by_extension("yaml"),
                "md" => ps.find_syntax_by_extension("markdown"),
                _ => None,
            }
        })
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);

    for line in LinesWithEndings::from(code) {
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, ps)?;
        let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
        write!(out, "{}", escaped)?;
        out.execute(SetAttribute(Attribute::Reset))?;
    }

    // Add blank line after code block
    writeln!(out)?;

    Ok(())
}

/// Streaming markdown renderer for real-time pretty-printing.
///
/// `PrettyPrinter` maintains internal state to accumulate incoming text chunks and
/// render complete markdown elements (lines, code blocks) as soon as they're formed.
/// This is ideal for displaying LLM responses as tokens stream in.
///
/// # State Machine
///
/// The printer operates as a state machine with two primary states:
///
/// 1. **Normal Mode**: Accumulating regular markdown text
/// 2. **Code Block Mode**: Accumulating code between ` ```lang ` and ` ``` `
///
/// ```text
/// ┌─────────────────┐
/// │  Normal Mode    │
/// │  (buffer text)  │
/// └────────┬────────┘
///          │ detect "```"
///          ▼
/// ┌─────────────────┐
/// │ Code Block Mode │
/// │ (buffer code)   │
/// └────────┬────────┘
///          │ detect "```"
///          ▼
/// ┌─────────────────┐
/// │  Print Block    │
/// │ (syntax highlight)
/// └─────────────────┘
/// ```
///
/// # Usage Pattern
///
/// ```no_run
/// use awful_aj::pretty::PrettyPrinter;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut printer = PrettyPrinter::new();
///
/// // Add chunks as they arrive
/// printer.add_chunk("# ")?;
/// printer.add_chunk("Title")?;
/// printer.add_chunk("\n")?;
///
/// // Flush remaining content
/// printer.flush()?;
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// - **Memory**: Buffers one line/code block at a time (minimal memory overhead)
/// - **Latency**: Prints complete elements immediately (low latency)
/// - **CPU**: Regex matching and syntax highlighting are amortized over chunks
///
/// # Examples
///
/// ## Basic Streaming
///
/// ```no_run
/// use awful_aj::pretty::PrettyPrinter;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut printer = PrettyPrinter::new();
///
/// let chunks = vec!["# ", "Header", "\n", "\n", "Text ", "line", "\n"];
/// for chunk in chunks {
///     printer.add_chunk(chunk)?;
/// }
///
/// printer.flush()?;
/// # Ok(())
/// # }
/// ```
///
/// ## Code Block Streaming
///
/// ```no_run
/// use awful_aj::pretty::PrettyPrinter;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut printer = PrettyPrinter::new();
///
/// printer.add_chunk("```")?;
/// printer.add_chunk("rust")?;
/// printer.add_chunk("\n")?;
/// printer.add_chunk("fn main() {\n")?;
/// printer.add_chunk("    println!(\"Hi\");\n")?;
/// printer.add_chunk("}\n")?;
/// printer.add_chunk("```")?;
///
/// printer.flush()?;
/// # Ok(())
/// # }
/// ```
pub struct PrettyPrinter {
    /// Buffer for accumulating incomplete text.
    buffer: String,
    /// Whether we're currently inside a code block.
    in_code_block: bool,
    /// Language identifier for the current code block (e.g., "rust", "python").
    code_language: String,
    /// Accumulated code content for the current code block.
    code_content: String,
}

impl PrettyPrinter {
    /// Create a new `PrettyPrinter` with empty buffers.
    ///
    /// # Returns
    ///
    /// A new printer instance ready to receive chunks.
    ///
    /// # Examples
    ///
    /// ```
    /// use awful_aj::pretty::PrettyPrinter;
    ///
    /// let printer = PrettyPrinter::new();
    /// ```
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            in_code_block: false,
            code_language: String::new(),
            code_content: String::new(),
        }
    }

    /// Add a text chunk and print any complete markdown elements.
    ///
    /// This method accumulates the chunk in an internal buffer and detects:
    /// - **Complete lines**: Printed immediately with markdown formatting
    /// - **Code block boundaries**: ` ``` ` markers trigger code block rendering
    /// - **Partial content**: Buffered until more chunks arrive
    ///
    /// # Parameters
    ///
    /// - `chunk`: Text fragment to add (may be partial word, line, or complete block)
    ///
    /// # Returns
    ///
    /// `Ok(())` if rendering succeeded, or an error if terminal output failed.
    ///
    /// # Behavior
    ///
    /// - **Outside code blocks**: Prints complete lines as markdown
    /// - **Inside code blocks**: Buffers code until closing ` ``` `
    /// - **Code block start**: Detects ` ```lang ` and switches to code mode
    /// - **Code block end**: Renders accumulated code with syntax highlighting
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use awful_aj::pretty::PrettyPrinter;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut printer = PrettyPrinter::new();
    ///
    /// printer.add_chunk("Hello ")?;
    /// printer.add_chunk("**world**")?;
    /// printer.add_chunk("!\n")?;  // Line complete, prints: "Hello world!"
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_chunk(&mut self, chunk: &str) -> Result<(), Box<dyn Error>> {
        self.buffer.push_str(chunk);

        // Check for code block markers
        if self.buffer.contains("```") {
            if !self.in_code_block {
                // Starting a code block
                if let Some(idx) = self.buffer.find("```") {
                    // Print everything before the code block
                    let before = &self.buffer[..idx];
                    if !before.is_empty() {
                        let mut out = stdout();
                        print_markdown(before, &mut out)?;
                    }

                    // Extract language and start collecting code
                    let after = &self.buffer[idx + 3..];
                    if let Some(newline_idx) = after.find('\n') {
                        self.code_language = after[..newline_idx].trim().to_string();
                        self.code_content = after[newline_idx + 1..].to_string();
                        self.in_code_block = true;
                        self.buffer.clear();
                    }
                }
            } else {
                // Ending a code block
                if let Some(idx) = self.buffer.find("```") {
                    self.code_content.push_str(&self.buffer[..idx]);

                    // Print the code block
                    let ps = SyntaxSet::load_defaults_newlines();
                    let ts = ThemeSet::load_defaults();
                    let theme = &ts.themes["base16-ocean.dark"];
                    let mut out = stdout();
                    print_code_block(&self.code_content, &self.code_language, &ps, theme, &mut out)?;

                    self.in_code_block = false;
                    self.code_language.clear();
                    self.code_content.clear();
                    self.buffer = self.buffer[idx + 3..].to_string();
                }
            }
        } else if self.in_code_block {
            // Accumulate code content
            self.code_content.push_str(&self.buffer);
            self.buffer.clear();
        } else {
            // Print complete lines
            while let Some(newline_idx) = self.buffer.find('\n') {
                let line = &self.buffer[..newline_idx];
                let mut out = stdout();
                print_markdown(line, &mut out)?;
                writeln!(out)?;
                self.buffer = self.buffer[newline_idx + 1..].to_string();
            }
        }

        Ok(())
    }

    /// Flush any remaining buffered content to the terminal.
    ///
    /// Call this after all chunks have been added to ensure partial lines or incomplete
    /// markdown elements are rendered. This is especially important at the end of streaming
    /// to display any text that didn't end with a newline.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the flush succeeded, or an error if terminal output failed.
    ///
    /// # Behavior
    ///
    /// - If buffer is empty: No-op (returns immediately)
    /// - If buffer contains text: Renders as markdown and clears buffer
    /// - If in code block: **Does not** render incomplete code block (call with closing ` ``` ` first)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use awful_aj::pretty::PrettyPrinter;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut printer = PrettyPrinter::new();
    ///
    /// printer.add_chunk("Partial line without newline")?;
    /// printer.flush()?;  // Ensures the partial line is printed
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Streaming Workflow
    ///
    /// ```no_run
    /// use awful_aj::pretty::PrettyPrinter;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let chunks = vec!["chunk1", "chunk2"];
    /// let mut printer = PrettyPrinter::new();
    ///
    /// // Process all chunks
    /// for chunk in chunks {
    ///     printer.add_chunk(chunk)?;
    /// }
    ///
    /// // IMPORTANT: Always flush at the end
    /// printer.flush()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.buffer.is_empty() {
            let mut out = stdout();
            print_markdown(&self.buffer, &mut out)?;
            self.buffer.clear();
        }
        Ok(())
    }
}

impl Default for PrettyPrinter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pretty_printer_creation() {
        let printer = PrettyPrinter::new();
        assert!(printer.buffer.is_empty());
        assert!(!printer.in_code_block);
        assert!(printer.code_language.is_empty());
        assert!(printer.code_content.is_empty());
    }

    #[test]
    fn test_pretty_printer_default() {
        let printer = PrettyPrinter::default();
        assert!(printer.buffer.is_empty());
        assert!(!printer.in_code_block);
    }

    #[test]
    fn test_pretty_printer_add_chunk() {
        let mut printer = PrettyPrinter::new();

        // Add simple text chunk
        let result = printer.add_chunk("Hello ");
        assert!(result.is_ok());
        assert_eq!(printer.buffer, "Hello ");

        // Add more text
        let result = printer.add_chunk("world");
        assert!(result.is_ok());
        assert_eq!(printer.buffer, "Hello world");
    }

    #[test]
    fn test_pretty_printer_flush_empty() {
        let mut printer = PrettyPrinter::new();
        let result = printer.flush();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pretty_printer_flush_with_content() {
        let mut printer = PrettyPrinter::new();
        printer.buffer = "Test content".to_string();

        let result = printer.flush();
        assert!(result.is_ok());
        assert!(printer.buffer.is_empty());
    }

    #[test]
    fn test_pretty_printer_code_block_state() {
        let mut printer = PrettyPrinter::new();

        // Start code block
        printer.in_code_block = true;
        printer.code_language = "rust".to_string();
        printer.code_content = "fn main() {}".to_string();

        assert!(printer.in_code_block);
        assert_eq!(printer.code_language, "rust");
        assert_eq!(printer.code_content, "fn main() {}");
    }

    #[test]
    fn test_print_pretty_with_simple_markdown() {
        // Test that print_pretty doesn't panic with simple markdown
        let markdown = "# Hello\n\nThis is **bold** text.";
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_code_block() {
        // Test that print_pretty doesn't panic with code blocks
        let markdown = r#"
# Test

```rust
fn main() {
    println!("Hello");
}
```

Normal text after code.
"#;
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_empty_string() {
        let result = print_pretty("");
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_inline_code() {
        let markdown = "Use the `println!` macro to print text.";
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_headers() {
        let markdown = r#"# Header 1
## Header 2
### Header 3
"#;
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_italic() {
        let markdown = "This is *italic* text.";
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_multiple_code_blocks() {
        let markdown = r#"
First block:

```python
print("Hello")
```

Second block:

```rust
println!("World");
```
"#;
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_unknown_language() {
        let markdown = r#"
```unknownlang
some code here
```
"#;
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_pretty_with_mixed_formatting() {
        let markdown = r#"# Title

This has **bold**, *italic*, and `code`.

```rust
fn test() {}
```

More text.
"#;
        let result = print_pretty(markdown);
        assert!(result.is_ok());
    }
}
