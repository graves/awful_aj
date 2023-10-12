# aj

`aj` is a command-line tool designed to facilitate the development of features on top of locally running Large Language Models (LLMs). It is specifically tailored to work seamlessly with the [Text Generation Web UI](https://github.com/oobabooga/text-generation-webui/) with the [OpenAI extension enabled](https://github.com/oobabooga/text-generation-webui/tree/main/extensions/openai), ensuring a smooth and efficient development experience without the need for complex configurations or setups.

## Features

- **Local Large Language Model Interaction**: Directly interact with your locally running LLM from the command line.
- **Seamless Integration**: Works out-of-the-box with the [Text Generation Web UI](https://github.com/oobabooga/text-generation-webui/) with the [OpenAI extension enabled](https://github.com/oobabooga/text-generation-webui/tree/main/extensions/openai).
- **Custom Templates**: Utilize pre-made templates for common queries or create your own for specialized tasks.
- **Rich, Colored Responses**: Enjoy interactive, colored, and rich-text responses directly in your terminal for an enhanced user experience.

## Installation

Install `aj` using `cargo`, the Rust package manager. If Rust and `cargo` are not already installed, get them from [rustup](https://rustup.rs/), and then install `aj`:
```sh
cargo install awful_aj
```

## Usage

### Initialization

Before you start using `aj`, initiate it to create the necessary configuration and template files:
```sh
aj init
```

This command creates folders at `~/.config/aj` and `~/.config/aj/templates` and populates them with default configurations and templates.

### Configuration

The configuration is stored in `~/.config/aj/config.yaml`. Update the `api_key` field with your actual API key before utilizing the aj tool. The initial configuration looks like this:
```yaml
api_base: "http://localhost:5001/v1"
api_key: "CHANGEME"
model: "mistrel-7b-openorca"
```

### Asking Questions

To ask a question, use the ask command followed by your question in quotes:
```sh
aj ask "Is Bibi really from Philly?"
```

If no question is provided, a default question is used.

### Templates

Templates reside in the `~/.config/aj/templates` directory. Feel free to add or modify templates as needed. A default template, `simple_question.yml`, is provided during initialization.

### Development

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

### Contributing

Contributions are welcome! Feel free to open a PR.

### License

awful_aj is under the [MIT License](LICENSE).