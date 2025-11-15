# RAG (Retrieval-Augmented Generation) 📚

RAG enhances the assistant's responses by injecting relevant snippets from your documents directly into the conversation context. This allows the model to answer questions about specific files, codebases, papers, or documentation without requiring fine-tuning.

## 🎯 What is RAG?

**Retrieval-Augmented Generation** is a technique that combines:
1. **Document retrieval** via semantic search (finding relevant text chunks)
2. **Context augmentation** (injecting those chunks into the LLM prompt)
3. **Generation** (the LLM uses the injected context to answer)

In `awful_aj`, RAG is implemented using:
- **Chunking**: Documents split into overlapping segments
- **Embeddings**: Each chunk converted to a vector using `all-MiniLM-L6-v2`
- **HNSW index**: Fast approximate nearest-neighbor search
- **Top-k retrieval**: Most relevant chunks injected into prompt preamble

## 🔧 How It Works

### Pipeline

```
┌─────────────────────┐
│ Your Documents      │
│ (plain text files)  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Chunking            │  512 tokens per chunk
│                     │  128 token overlap
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Sentence Embeddings │  all-MiniLM-L6-v2
│                     │  384-dimensional vectors
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ HNSW Vector Index   │  Temporary in-memory store
│                     │  (not persisted)
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Query Time:         │
│ 1. Embed question   │
│ 2. Find top-k       │
│ 3. Inject into      │
│    prompt preamble  │
└─────────────────────┘
```

### Chunking Strategy

- **Chunk size**: 512 tokens (approximate, based on GPT-2 tokenizer)
- **Overlap**: 128 tokens between consecutive chunks
- **Why overlap?**: Prevents splitting important context across chunk boundaries

Example:
```
Document: "The quick brown fox jumps over the lazy dog. [... 500 more tokens ...]"

Chunk 1: tokens 0-512
Chunk 2: tokens 384-896  (overlaps with chunk 1 by 128 tokens)
Chunk 3: tokens 768-1280 (overlaps with chunk 2 by 128 tokens)
```

### Embedding Model

`awful_aj` uses **`all-MiniLM-L6-v2`** from the sentence-transformers library:
- **Dimensions**: 384
- **Speed**: Fast (~40ms per chunk on CPU)
- **Quality**: Good balance of speed and semantic understanding
- **License**: Apache 2.0

This model converts text into dense vectors where semantically similar text has similar vectors (measured by cosine similarity / Euclidean distance).

### HNSW Indexing

**Hierarchical Navigable Small World (HNSW)** is a graph-based algorithm for approximate nearest-neighbor search:
- **Construction**: Builds a multi-layer graph with shortcuts
- **Search**: Logarithmic time complexity
- **Trade-off**: Approximate (not exhaustive), but extremely fast

When you provide RAG files, `awful_aj` builds a **temporary** HNSW index in memory. This index is **not persisted** to disk (unlike the conversation VectorStore).

### Retrieval at Query Time

When you ask a question:
1. Your question is embedded into a 384-dimensional vector
2. HNSW finds the `k` most similar chunk vectors (default: 3)
3. The corresponding text chunks are extracted
4. Chunks are injected into the prompt **preamble** (before your question)

The LLM sees:
```
[System prompt]

Context from documents:
---
Chunk 1: "..."
Chunk 2: "..."
Chunk 3: "..."
---

[Your question]
```

## 🚀 Usage

### Basic RAG Query

```bash
# Ask about a single file
aj ask -r README.md "How do I install this?"

# Multiple files (comma-separated)
aj ask -r "docs/api.md,docs/tutorial.md" "Explain authentication"
```

### Controlling Retrieval

```bash
# Retrieve more chunks for complex questions
aj ask -r paper.txt -k 10 "Summarize the entire methodology"

# Fewer chunks for focused questions
aj ask -r config.yaml -k 1 "What's the default port?"
```

### Interactive RAG Sessions

```bash
# Load documents once, query many times
aj interactive -r "docs/*.md"

# With session persistence
aj interactive -s my-project -r "README.md,CHANGELOG.md"
```

The interactive mode loads RAG documents **once at startup** and reuses the index for all queries in that session.

## 📊 RAG vs. VectorStore (Memories)

`awful_aj` has **two separate vector stores**:

| Feature | RAG (Temporary) | VectorStore (Persistent) |
|---------|-----------------|--------------------------|
| **Purpose** | Document-based Q&A | Conversation memory |
| **Source** | User-provided files (`-r` flag) | Past conversation messages |
| **Lifetime** | Single invocation | Persisted to `<session>_vector_store.yaml` |
| **Indexing** | On-demand at startup | Incremental as messages evicted from Brain |
| **Retrieval** | Every query | Loaded into Brain preamble |
| **Use case** | "Explain this codebase" | "Remember what we discussed last week" |

### When to Use RAG

✅ **Use RAG when**:
- Asking about external documents/code
- You need answers grounded in specific files
- Documents change frequently (no need to retrain)
- One-off queries about large documents

❌ **Don't use RAG for**:
- General knowledge questions (no documents needed)
- Conversation continuity (use sessions instead)
- Real-time data (RAG uses static files)

### Combining RAG and Sessions

You can use **both** together:

```bash
# Session provides conversation memory
# RAG provides document context
aj ask -s research -r paper.pdf "Relate this to what we discussed yesterday"
```

The LLM will see:
1. **Relevant past messages** (from session VectorStore)
2. **Relevant document chunks** (from RAG)
3. **Your current question**

## ⚙️ Technical Details

### File Requirements

RAG accepts **plain text files only**:
- ✅ `.txt`, `.md`, `.rs`, `.py`, `.js`, `.json`, `.yaml`, etc.
- ❌ Binary formats (PDF, Word, etc.) are **not supported**

> **Tip**: Use tools like `pdftotext` to convert PDFs to plain text first.

### Memory Usage

Each chunk requires:
- **Embedding**: 384 floats × 4 bytes = 1.5 KB
- **Text storage**: Variable (original text)
- **HNSW index**: ~10 bytes per node (overhead)

Example: A 100-page document (~50,000 tokens) produces:
- ~100 chunks (512 tokens each with overlap)
- ~150 KB of embeddings
- ~1-2 MB of HNSW graph

### Performance

| Operation | Time (CPU) | Notes |
|-----------|-----------|-------|
| Embed 1 chunk | ~40ms | `all-MiniLM-L6-v2` on M1 Mac |
| Build HNSW (100 chunks) | ~5s | Includes embedding time |
| Query HNSW | <1ms | Logarithmic search |

### Token Budget

RAG chunks are injected into the **prompt preamble**, which consumes part of your context window:

- 3 chunks × 512 tokens = **~1536 tokens** (default)
- 10 chunks × 512 tokens = **~5120 tokens**

Ensure your `context_max_tokens` is large enough to accommodate:
```
system_prompt + rag_chunks + conversation_history + user_question + assistant_response
```

## 💡 Best Practices

### 1. Use Descriptive File Names

The RAG system doesn't currently inject file names as metadata, so choose files whose content is self-contained.

### 2. Chunk Size Matters

The default 512-token chunks work well for most cases:
- **Larger chunks**: More context per retrieval, but fewer total chunks
- **Smaller chunks**: More granular retrieval, but may miss context

> Current implementation has **fixed** chunk size (512 tokens, 128 overlap). Future versions may expose this as a parameter.

### 3. Ask Specific Questions

RAG works best with targeted queries:
- ✅ "What are the installation steps for Linux?"
- ❌ "Tell me about this project" (too broad)

### 4. Combine with Sessions

Use sessions to build on previous RAG queries:
```bash
aj interactive -s codebase-exploration -r "src/**/*.rs"

You: "What does the main function do?"
Assistant: [uses RAG to find main.rs]

You: "How does it call the parser?"
Assistant: [uses session memory + RAG]
```

### 5. Monitor Token Usage

If you set `-k` too high, you may exhaust the context window:
```bash
# Dangerous with small context windows
aj ask -r huge-document.txt -k 50 "..."
```

Check your `context_max_tokens` in `config.yaml` and adjust `-k` accordingly.

## 🔮 Future Enhancements

Potential improvements to the RAG system:

- [ ] **Configurable chunk size**: `--rag-chunk-size` flag
- [ ] **PDF support**: Auto-convert PDFs using `poppler`
- [ ] **Persistent RAG index**: Cache embeddings across invocations
- [ ] **Web scraping**: `--rag-url` to fetch and chunk web pages
- [ ] **Recursive directory loading**: `--rag-dir src/` to load all files

## 📚 Examples

### Code Documentation

```bash
# Understand a Rust module
aj ask -r src/vector_store.rs "How does HNSW indexing work here?"

# Compare implementations
aj ask -r "src/api.rs,src/session_messages.rs" \
  "How do these modules interact?"
```

### Research Papers

```bash
# Summarize methodology
aj ask -r paper.txt -k 5 "Explain the experimental setup"

# Extract specific details
aj ask -r paper.txt "What datasets were used?"
```

### Project Onboarding

```bash
# Interactive exploration
aj interactive -r "README.md,CONTRIBUTING.md,docs/architecture.md" -p

You: "How do I get started?"
You: "What's the testing strategy?"
You: "Where is the configuration stored?"
```

### Configuration Files

```bash
# Query YAML configs
aj ask -r config.yaml "What's the default API endpoint?"

# Multi-file config analysis
aj ask -r "docker-compose.yml,Dockerfile,.env.example" \
  "How is the database configured?"
```

## 🐛 Troubleshooting

### "Failed to read RAG file"

Ensure the file path is correct and the file exists:
```bash
# Absolute path
aj ask -r /full/path/to/document.txt "..."

# Relative to CWD
aj ask -r ./docs/guide.md "..."
```

### "Context window exceeded"

Reduce `-k` or use a model with larger context:
```bash
# Fewer chunks
aj ask -r large-doc.txt -k 2 "..."

# Or increase context_max_tokens in config.yaml
context_max_tokens: 16384
```

### Empty Responses

RAG retrieval may not find relevant chunks if:
- Question is too vague
- Document doesn't contain relevant information
- Chunk size is too small (splits context)

Try rephrasing your question or checking chunk boundaries.

### Binary File Error

RAG only supports plain text. Convert binary files first:
```bash
# PDF to text
pdftotext paper.pdf paper.txt
aj ask -r paper.txt "..."

# Word to text (macOS)
textutil -convert txt document.docx
aj ask -r document.txt "..."
```

## 🎓 Learn More

- **HNSW Algorithm**: [arXiv:1603.09320](https://arxiv.org/abs/1603.09320)
- **Sentence Transformers**: [sbert.net](https://www.sbert.net/)
- **RAG Paper**: [arXiv:2005.11401](https://arxiv.org/abs/2005.11401)
- **awful_aj Memories**: [memories.md](./memories.md) for persistent vector store details
