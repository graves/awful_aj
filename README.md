# aj

[![crates.io](https://img.shields.io/crates/v/awful_aj.svg)](https://crates.io/crates/awful_aj)
[![docs.rs](https://docs.rs/awful_aj/badge.svg)](https://docs.rs/awful_aj)

`aj` is a command-line tool designed to facilitate the development of features on top of locally running Large Language Models (LLMs). It is specifically tailored to work seamlessly with the [Text Generation Web UI](https://github.com/oobabooga/text-generation-webui/) with the [OpenAI extension enabled](https://github.com/oobabooga/text-generation-webui/tree/main/extensions/openai), ensuring a smooth and efficient development experience without the need for complex configurations or setups.

![Awful Jade CLI tool logo](aj.png)

## Features

- **Local Large Language Model Interaction**: Directly interact with your locally running LLM from the command line.
- **Seamless Integration**: Works out-of-the-box with the [Text Generation Web UI](https://github.com/oobabooga/text-generation-webui/) with the [OpenAI extension enabled](https://github.com/oobabooga/text-generation-webui/tree/main/extensions/openai).
- **Custom Templates**: Utilize pre-made templates for common queries or create your own for specialized tasks.
- **Rich, Colored Responses**: Enjoy interactive, colored, and rich-text responses directly in your terminal for an enhanced user experience.

## Installation

For now `aj` requires `libtorch` to be installed independently. Get the correct version of `libtorch` from [pytorch](https://download.pytorch.org/libtorch/cu124/).

### Linux:

```bash
export LIBTORCH=/path/to/libtorch
export LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH
```

### Windows

```powershell
$Env:LIBTORCH = "X:\path\to\libtorch"
$Env:Path += ";X:\path\to\libtorch\lib"
```

### macOS + Homebrew

#### bash

```bash
brew install pytorch jq
export LIBTORCH=$(brew --cellar pytorch)/$(brew info --json pytorch | jq -r '.[0].installed[0].version')
export DYLD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH
```

#### nushell

```nushell
brew install pytorch jq
$env.LIBTORCH = $"(brew --cellar pytorch)/(brew info --json pytorch | jq -r '.[0].installed[0].version')"
$env.DYLD_LIBRARY_PATH = $"($env.LIBTORCH)/lib"
```

### Platform agnostic

Install `aj` using `cargo`, the Rust package manager. If Rust and `cargo` are not already installed, get them from [rustup](https://rustup.rs/), and then install `aj`:
```sh
cargo install --git https://github.com/graves/awful_aj
```

## Usage

### Initialization

Before you start using `aj`, initiate it to create the necessary configuration and template files:
```sh
aj init
```

Then run the diesel database migrations. The database URL will change depending on where your database is initialized. `aj` uses the `dirs_rs` library to determine the platforms configuration directory.
```sh
diesel database reset --database-url "/Users/tg/Library/Application Support/com.awful-sec.aj/aj.db"
```

This command creates files and folders in the platforms configuration directory and populates them with default configurations and templates.

### Configuration

The configuration is stored in `~/.config/aj/config.yaml`. Update the `api_key` field with your actual API key before utilizing the `aj` tool. The initial configuration looks like this:
```yaml
api_base: "http://localhost:1234/v1"
api_key: "CHANGEME"
model: "hermes-2-pro-mistral-7b"
context_max_tokens: 8192
assistant_minimum_context_tokens: 2048
stop_words: ["<|im_end|>\\n<|im_start|>", "<|im_start|>\n"]
session_db_url: "/Users/tg/Library/Application Support/com.awful-sec.aj/aj.db"
```

### Asking Questions

To ask a question, use the ask command followed by your question in quotes:
```sh
aj ask "Is Bibi (Netanyahu) really from Philly?"
```

If no question is provided, a default question is used.

### Templates

Templates reside in the `~/.config/aj/templates` directory. Feel free to add or modify templates as needed. A default template, `simple_question.yml`, is provided during initialization.

## Development

Clone the repository:
```sh
git clone https://github.com/graves/awful_aj.git
cd awful_aj
```
Build the project:
```sh
cargo build
```
Run tests:
```sh
cargo test
```

When running the test suite you can safely ignore the following error:
```
2023-10-12T21:08:39.726156Z ERROR aj::api: Received error: stream failed: Invalid header value: "application/json"
error: stream failed: Invalid header value: "application/json"
```

## Contributing

Contributions are welcome! Feel free to open a PR.

## License

awful_aj is under the [MIT License](LICENSE).