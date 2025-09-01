# Config ⚙️

`aj` reads a YAML configuration file from your platform directory.

## 📍 Paths
- **macOS**: `~/Library/Application Support/com.awful-sec.aj/config.yaml`
- **Linux**: `~/.config/aj/config.yaml`
- **Windows**: `C:\\Users\\YOU\\AppData\\Roaming\\awful-sec\\aj\\config.yaml`

## 🧾 Example
```yaml
api_base: "http://localhost:1234/v1"
api_key: "CHANGEME"
model: "jade_qwen3_4b_mlx"
context_max_tokens: 8192
assistant_minimum_context_tokens: 2048
stop_words:
  - "<|im_end|>\\n<|im_start|>"
  - "<|im_start|>\n"
session_db_url: "/Users/you/Library/Application Support/com.awful-sec.aj/aj.db"
should_stream: true
```

## 🔑 Key Fields
- `api_base`: Where requests go.
- `api_key`: Secret authorization key (optional).
- `model`: LLM model to use.
- `context_max_tokens`: Context length to request from model.
- `assistant_minimum_context_tokens`: Will always eject messages from the context if needed to make room for this many tokens to fit the response.
- `stop_words`: Tokens to cut off model output.
- `session_db_url`: SQLite DB path for sessions (optional).
= `should_stream`: Whether to stream the response or return it all at once when the inference ends.

## ✍️ Editing Tips
- After edits, re‑run your command (no daemon reloads required).
- Make sure you include the port number your LLM inference server is running on.
- If your using an online service you can usually create an API key on your account settings or api page.
- `jade_qwen3_4b_mlx` is a highly capable Qwen 3 4B model that I finetuned for Apple focused systems programming and general discussion. You can find it on [huggingface](https://huggingface.co/dougiefresh/jade_qwen3_4b) or download it directly in LM Studio.