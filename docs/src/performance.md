# Performance Considerations ⚡

This guide covers performance optimization and resource management for `aj`.

## 📊 Resource Requirements

### Minimum Requirements
- **RAM**: 2GB for basic usage
- **Storage**: 500MB for embeddings model + cache
- **CPU**: Any modern processor (embedding computation)

### Recommended for Large Projects
- **RAM**: 8GB+ for large document processing
- **Storage**: 2GB+ for extensive RAG cache
- **CPU**: Multi-core for parallel embedding computation

### Model-Specific Requirements

#### Embedding Model (`all-MiniLM-L6-v2`)
- **Model size**: ~40MB
- **RAM during inference**: ~200MB
- **Cache storage**: Variable, based on document size

#### LLM Models
- **Local models**: Varies by model size and framework
- **API models**: Network latency + response time
- **Streaming**: Lower initial latency, continuous resource use

## 🚀 Performance Optimization

### Memory Management

#### Token Budgeting
Optimize context window usage:

```yaml
# ~/.config/aj/config.yaml
context_max_tokens: 8192          # Total context window
assistant_minimum_context_tokens: 2048  # Reserve for response
```

**Tips:**
- Smaller `context_max_tokens` = faster responses
- Larger `assistant_minimum_context_tokens` = safer responses
- Adjust based on your model's capabilities

#### Memory-Efficient Sessions

```bash
# Use smaller sessions for focused work
aj interactive -s focused-session --context-tokens 2048

# Clear memory between topics
aj reset
aj interactive -s new-topic

# Use one-shot for simple queries
aj ask "simple question" --one-shot
```

### RAG Performance

#### Document Chunking Strategy

Default chunking: 512 tokens with 128 token overlap

```bash
# Optimize for different document types
aj ask -r code.rs -k 3 -c 256 -o 64 "Analyze this function"  # Smaller chunks for code
aj ask -r paper.pdf -k 5 -c 1024 -o 256 "Summarize methodology"  # Larger chunks for text
```

#### Cache Optimization

```bash
# Warm up cache for frequently used documents
aj ask -r "api.md,guide.md" -k 10 "Load and cache"

# Subsequent queries use cache (much faster)
aj ask -r "api.md" "What are the endpoints?"
```

#### Batch Processing

```bash
# Process multiple documents efficiently
aj interactive -s batch-session -r "docs/,specs/" -p

# Multiple queries in same session (documents loaded once)
> What are the main features?
> How does authentication work?
> What are the performance characteristics?
```

### Embedding Performance

#### Model Selection

```yaml
# Choose based on use case
embedding_model: "all-MiniLM-L6-v2"          # Fast, general purpose
# embedding_model: "all-mpnet-base-v2"      # Better quality, slower
# embedding_model: "multi-qa-mpnet-base"    # Best for Q&A
```

#### Parallel Processing

`aj` automatically parallelizes embedding computation when possible:

```bash
# Multi-core utilization for large documents
aj ask -r large_corpus/ -k 15 "Comprehensive analysis"
```

## 🔍 Performance Monitoring

### Resource Usage Monitoring

#### Memory Usage
```bash
# Monitor RSS memory during operation
/usr/bin/time -v aj ask -r large_document.txt "Summarize"

# Check memory leaks in interactive sessions
RUST_LOG=debug aj interactive -s memory-test
```

#### CPU Usage
```bash
# Profile CPU usage
perf record --call-graph=dwarf aj ask -r codebase/ "Analyze"
perf report

# Or use simpler tools
htop  # Monitor during aj operations
```

#### Network Usage
```bash
# Monitor API calls
RUST_LOG=aj::api=debug aj ask "test question"

# Check embedding downloads
curl -v https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
```

### Performance Monitoring

#### Cache Effectiveness
Monitor RAG cache behavior:
```bash
# First query processes documents
aj ask -r document.txt "test" -o

# Second query uses cached chunks when possible
aj ask -r document.txt "another question" -o
```

## 🛠️ Performance Tuning

### Configuration Optimization

#### High-Performance Config
```yaml
# For maximum performance
api_base: "http://localhost:1234/v1"  # Fast local API
model: "fast-model"                   # Optimized for speed
context_max_tokens: 4096              # Smaller context
should_stream: true                   # Streaming responses
embedding_model: "all-MiniLM-L6-v2"   # Fast embeddings
```

#### Quality-Focused Config
```yaml
# For maximum quality
api_base: "https://api.openai.com/v1"  # High-quality API
model: "gpt-4"                         # Best model
context_max_tokens: 16384              # Large context
should_stream: false                   # Complete responses
embedding_model: "all-mpnet-base-v2"   # Better embeddings
```

### System-Level Optimization

#### Memory Management
```bash
# Monitor and tune system memory
sudo sysctl vm.swappiness=10  # Reduce swapping

# For large documents, consider increasing swap
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

#### Storage Optimization
```bash
# Use SSD for better cache performance
# Place cache on fast storage if possible
export AJ_CACHE_DIR="/fast/ssd/aj_cache"

# Clean old cache files
find ~/.config/aj/rag_cache/ -type f -atime +7 -delete
```

## 📈 Performance Characteristics

### Factors Affecting Performance

#### Document Processing
- **Document size**: Larger documents require more embedding computation time
- **Chunk overlap**: Default 128 token overlap affects processing speed
- **Cache hits**: Subsequent queries on same documents are faster
- **Parallel processing**: Multi-core CPUs improve embedding speed

#### Memory Usage
- **Embedding model**: Requires memory for model loading and inference
- **Vector store**: Grows with number of document chunks
- **Session memory**: Accumulates conversation tokens over time
- **RAG cache**: Stores processed chunks for faster retrieval

### Performance Tips by Use Case

#### Development Workflows
```bash
# Fast iteration for coding
aj interactive -s dev --context-tokens 2048 -r "src/" -k 3

# Use streaming for immediate feedback
aj ask "debug this" -p --stream
```

#### Document Analysis
```bash
# Batch process multiple documents
for file in docs/*.md; do
  aj ask -r "$file" "Summarize key points" -o
done

# Deep analysis with larger context
aj interactive -s analysis --context-tokens 16384 -r "papers/" -k 10
```

#### Research Workflows
```bash
# Load research corpus once
aj interactive -s research -r "papers/,articles/" -k 15 -p

# Multiple questions across corpus
> What are the main themes?
> What are the contradictions?
> What research gaps exist?
```

## 🔧 Troubleshooting Performance Issues

### Common Performance Problems

#### Slow First Query
**Problem**: First RAG query is very slow
**Solution**: Documents need embedding computation
```bash
# Pre-warm cache
aj ask -r all_documents/ "Load and cache" -o
```

#### Memory Growth
**Problem**: Memory usage grows over time
**Solution**: Session memory accumulation
```bash
# Reset periodically
aj reset

# Use smaller sessions
aj interactive -s small-session --context-tokens 2048
```

#### Cache Bloat
**Problem**: RAG cache grows too large
**Solution**: Clean old cache files
```bash
# Clean cache older than 7 days
find ~/.config/aj/rag_cache/ -type f -atime +7 -delete

# Or reset entirely
rm -rf ~/.config/aj/rag_cache/
```

### Performance Debugging

#### Enable Performance Logging
```bash
# Detailed performance metrics
RUST_LOG=aj::performance=trace aj ask "test question"

# Memory usage tracking
RUST_LOG=aj::memory=debug aj interactive -s perf-test
```

#### Profile Specific Operations
```bash
# Profile RAG performance
RUST_LOG=aj::rag=trace aj ask -r document.txt "test"

# Profile LLM calls
RUST_LOG=aj::api=debug aj ask "test question"
```

## 💡 Performance Best Practices

### General Guidelines
1. **Use appropriate context sizes** - smaller for focused questions, larger for analysis
2. **Leverage caching** - load documents once, query multiple times
3. **Monitor resource usage** - adjust settings based on your hardware
4. **Use streaming for interactive use** - better user experience
5. **Batch similar operations** - reduce overhead

### Use-Case Specific
- **Development**: Small sessions, fast responses, streaming
- **Research**: Large contexts, comprehensive RAG, batch processing
- **Documentation**: Medium contexts, structured output, template-driven
- **CI/CD**: One-shot mode, minimal resources, fast failure

### Hardware Optimization
- **SSD storage** for RAG cache
- **Sufficient RAM** for your document sizes
- **Multi-core CPU** for parallel embedding
- **Fast network** for API-based models

By following these performance considerations, you can optimize `aj` for your specific use case and hardware environment.
