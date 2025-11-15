# Memories 🧠

`aj` augments conversations with two complementary memory layers:
1. Working memory ("`Brain`") — a small, token-bounded queue of recent snippets used to build a preamble for each request.
2. Long-term memory ("`VectorStore`") — a persistent HNSW index of embeddings (MiniLM, 384-d) used for semantic recall.

Together they let AJ remember enough to be helpful, without blowing your context window. 🪄

- **Embeddings**: `all-mini-lm-l12-v2` (downloaded automatically).
- **Index**: HNSW for fast nearest‑neighbor lookups.
- **Policy**: Respect your token limits — prune oldest context when needed.

> **Note**: This memory system is for **conversation history**. For document-based question answering (e.g., "explain this codebase"), see [RAG (Retrieval-Augmented Generation)](./rag.md), which uses a separate temporary vector store for user-provided files.

## 🔬 How it Works
1. Your conversation text is embedded into vectors and stored.
2. At answer time, `aj` retrieves top‑K relevant snippets.
3. These snippets are stitched into context (bounded by `context_max_tokens`).

## 🎛️ Tuning Dials
- `context_max_tokens`: Overall window size.
- `assistant_minimum_context_tokens`: How much assistant context to preserve for responses.

## 🧩 Architecture at a Glance
- Brain (in-process):
    - Holds `VecDeque<Memory>` (role + content).
    - Enforces a token budget (`max_tokens`); evicts oldest entries when over.
- Builds a standardized preamble (3 messages):
    - system = template’s system_prompt
    - user = serialized brain JSON (a short explanatory line + `{"about", "memories":[...]}`
    - assistant = "Ok" (handshake/ack)
- VectorStore (persistent):
    - Embeds text via all-mini-lm-l12-v2 ➜ 384-d vectors.
    - Stores vectors in HNSW ([hora](https://github.com/hora-search/hora)) and maps ID → `Memory`.
    - Serialize to YAML + binary index (`<uuid>_hnsw_index.bin` under `config_dir()`).
    - Reloads the embedding model from `config_dir()/all-mini-lm-l12-v2` on deserialization.
- Sessions & Ejection:
    - When a rolling conversation exceeds budget, oldest user/assistant pair is ejected.
    - If a `VectorStore` is provided, those ejected turns are embedded + added to the index, then `build()` is called.
    - New questions trigger nearest-neighbor recall; relevant memories get pushed into the `Brain` before the request.

## 🔬 What Happens on Each `ask(...)`
- Session prep
- `get_session_messages(...)` loads/creates session state (DB-backed if session_name is set).
- Semantic recall
- `add_memories_to_brain(...)`:
    - Embed the current question.
    - Query HNSW for top-3 neighbors (search_nodes).
    - For each neighbor with Euclidean distance < 1.0, push its `Memory` into the `Brain`.
    - Rebuild the Brain preamble and update session preamble messages.
    - Preamble + prompt shaping
- Apply `pre_user_message_content` and/or `post_user_message_content` from `ChatTemplate`.
- Completion
    - If `should_stream == Some(true)`: `stream_response` prints blue/bold tokens live.
    - Else: `fetch_response` aggregates the content once.
- Persistence
    - Assistant reply is stored in the session DB (if sessions enabled).
    - If the rolling conversation later overflows: oldest pair is ejected, embedded, added to VectorStore, and the index is rebuilt.

## 🛠️ Minimal Setup
```rust
use awful_aj::{
  api, brain::Brain,
  config::AwfulJadeConfig,
  template::ChatTemplate,
  vector_store::VectorStore,
};

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = AwfulJadeConfig {
    api_key: "KEY".into(),
    api_base: "http://localhost:5001/v1".into(),
    model: "jade_qwen3_4b".into(),
    context_max_tokens: 8192,
    assistant_minimum_context_tokens: 2048,
    stop_words: vec![],
    session_db_url: "aj.db".into(),
    session_name: Some("memories-demo".into()), // ✅ enable sessions
    should_stream: Some(false),
    };

    let tpl = ChatTemplate {
    system_prompt: "You are Awful Jade. Use recalled notes if relevant. Be concise.".into(),
    messages: vec![],
    response_format: None,
    pre_user_message_content: None,
    post_user_message_content: None,
    };

    // Long-term memory store (requires MiniLM at config_dir()/all-mini-lm-l12-v2)
    let mut store = VectorStore::new(384, "memories-demo".into())?;

    // Working memory (brain) with its own token budget
    let mut brain = Brain::new(8092, &tpl);

    // Ask a question; add_memories_to_brain will auto-inject relevant neighbors
    let answer = api::ask(&cfg, "What is our project codename?".into(), &tpl,
                        Some(&mut store), Some(&mut brain)).await?;

    println!("{answer}");

    Ok(()) 
}
```

✅ Remember: After inserts to the `VectorStore`, call `build()` to make them searchable.

## 🧱 Seeding & Persisting the `VectorStore`

Seed once, then reuse across runs by deserializing.

```rust
use async_openai::types::Role;
use awful_aj::{brain::Memory, vector_store::VectorStore};
use std::path::PathBuf;

fn seed() -> Result<(), Box<dyn std::error::Error>> {
    let mut vs = VectorStore::new(384, "memories-demo".into())?;

    // Add whatever you want AJ to recall later:
    for s in [
    "Project codename is Alabaster.",
    "Primary repo is awful_aj owned by graves.",
    ] {
        let v = vs.embed_text_to_vector(s)?;
        vs.add_vector_with_content(v, Memory::new(Role::User, s.to_string()))?;
    }

    vs.build()?; // 🔔 finalize the index

    // Persist metadata (YAML) and the HNSW index (binary)
    vs.serialize(&PathBuf::from("vector_store.yaml"), "memories-demo".into())?;

    Reload later:

    use awful_aj::vector_store::VectorStore;

    # fn load() -> Result<VectorStore, Box<dyn std::error::Error>> {
    let yaml = std::fs::read_to_string("vector_store.yaml")?;
    let vs: VectorStore = serde_yaml::from_str(&yaml)?; // reload model + HNSW under the hood

    Ok(vs)
}
```

## 🎛️ Tuning Dials
- `context_max_tokens` (config): hard ceiling for the request construction.
- `assistant_minimum_context_tokens` (config): budget for assistant-side context within your flow.
- `Brain::max_tokens`: separate budget for the working memory JSON envelope.
- `Vector` recall: fixed to top-3 neighbors; include a memory if distance < 1.0 (Euclidean).
- Stop words: forwarded to the model; useful to avoid run-ons.
- Streaming: set `should_stream = Some(true)` for token-by-token prints.

🧪 If you frequently fail to recall useful notes, consider:
- Seeding more atomic memories (short, self-contained sentences).
- Lowering the distance threshold a bit (more inclusive), or raising it (more precise).
- Ensuring you rebuilt (`build()`) after inserts.
- Verifying the model path exists under `config_dir()/all-mini-lm-l12-v2`.

## 🧠 How the `Brain` Builds the Preamble

Every request gets a consistent, compact preamble:
- System — `template.system_prompt`
- User — a short paragraph + serialized brain JSON:
```json
{
  "about": "This JSON object is a representation of our conversation leading up to this point. This object represents your memories.",
  "memories": [
    {"role":"user","content":"..."},
    {"role":"assistant","content":"..."}
  ]
}
```
- Assistant — "Ok" (explicit acknowledgment)

This handshake primes the model with the latest, budget-friendly state before your new user message.

⛑️ Eviction: When the brain is over budget, it evicts oldest first and rebuilds the preamble. (Current implementation computes token count once; if you expect heavy churn, recomputing inside the loop would enforce the limit more strictly.)

## 🔁 Ejection → Embedding → Recall

When conversation history grows too large:
- Oldest user+assistant pair is ejected from session_messages.
- If a `VectorStore` is present:
    - Each piece is embedded, assigned an ID, and added to the HNSW index.
    - `build()` is called so they become searchable.
    - On the next `ask(...)`, the current question is embedded, top-3 neighbors are fetched, and any with distance < 1.0 get pushed into the Brain as memories.

Effect: older turns become semantic breadcrumbs you can recall later. 🍞🧭

## 🧰 Recipes

1. “Pin a fact” for later.

Drop a fact into the store right now so future questions recall it.
```rust
use async_openai::types::Role;
use awful_aj::{brain::Memory, vector_store::VectorStore};

fn pin(mut store: VectorStore) -> Result<(), Box<dyn std::error::Error>> {
    let fact = "Billing portal lives at https://hackme.example.com.";
    let v = store.embed_text_to_vector(fact)?;
    store.add_vector_with_content(v, Memory::new(Role::User, fact.into()))?;
    store.build()?; // make it queryable

    Ok(())
}
```

2. "Cold start" with a loaded brain.

Start a session by injecting a few memories before the first question.
```rust
use async_openai::types::Role;
use awful_aj::{brain::{Brain, Memory}, template::ChatTemplate};
use awful_aj::session_messages::SessionMessages;

fn warmup(mut brain: Brain, tpl: &ChatTemplate) -> Result<(), Box<dyn std::error::Error>> {
    let mut sess = SessionMessages::new(/* your cfg */ todo!());

    for seed in ["You are AJ.", "User prefers concise answers."] {
        brain.add_memory(Memory::new(Role::User, seed.into()), &mut sess);
    }

    let preamble = brain.build_preamble()?; // now ready
    assert!(!preamble.is_empty());

    Ok(())
}
}
```

## 🪵 Logging & Debugging
- Enable tracing to see:
    - brain token enforcement logs
    - serialized brain JSON
    - streaming events and request metadata (debug)
- If the model prints nothing in streaming mode, confirm your terminal supports ANSI and that stdout isn’t redirected without a TTY.
- If deserialization fails, verify:
    - `vector_store.yaml` exists and points to a matching `<uuid>_hnsw_index.bin` in `config_dir()`.
    - `all-mini-lm-l12-v2` is present (e.g., after `aj ask "Hello world!"`).

## 🔐 Privacy

Everything runs local by default:
- Embeddings and HNSW files live under your platform config dir (config_dir()).
- Session DB is local.
- Only your configured model endpoint receives requests.

## ✅ Quick Checklist
- Place MiniLM at `config_dir()/all-mini-lm-l12-v2` (or run your installer).
- Use `VectorStore::new(384, session_name);` after inserts, call `build()`.
- Enable sessions with `session_name: Some(...)` for ejection/persistence.
- Provide `Some(&mut store)`, `Some(&mut brain)` to `api::ask(...)` for semantic recall.
- Tune `context_max_tokens`, `assistant_minimum_context_tokens`, and `Brain::max_tokens`.
- (Optional) Set a JSON schema on `template.response_format` for structured replies.

> **Privacy note**: Everything is local by default. Keep secrets… consensual. 🤫