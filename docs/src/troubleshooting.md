# Troubleshooting 🔧

This guide covers common issues and solutions when using `aj`.

## 🚨 Common Issues

### Model Not Found
**Problem**: `Error: Failed to load config at <path>: <error_message>`

- `Failed to load config at /path/to/config.yaml: Invalid YAML syntax`
- `Failed to create client: <openai_error>` (from async-openai crate)
- `Error connecting to <api_base>` (network connection failure)
- `Error creating completion request: <model_specific_error>`

**Solutions**:
- **Config loading errors**:
  ```bash
  # Check config syntax
  aj --help  # Verify config path and syntax
  aj init --overwrite  # Recreate with defaults
  ```
  
- **Model/API errors**:
  ```bash
  # Test API connectivity
  curl http://localhost:1234/v1/models  # Verify server is running
  aj ask "test" --one-shot  # Test without session complexity
  ```

- **Common fixes**:
  - Update `api_base` to include full path: `http://localhost:1234/v1`
  - Verify `model` matches exactly what your server expects
  - Check `api_key` - leave empty for local models, use proper format for APIs

### Embedding Download Fails
**Problem**: `Failed to download embedding model from HuggingFace`

**Common Error Messages**:
- `Failed to load tokenizer: <tokenizer_error>` (from tokenizers crate)
- `Error connecting to huggingface.co` (network failure)
- `Failed to download config.json/tokenizer.json/model.safetensors` (file download errors)
- `Error creating client: <embedding_load_error>` (from SentenceEmbeddingsModel::load)

**Solutions**:
- **Network issues**:
  ```bash
  # Test HuggingFace connectivity
  curl -I https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/config.json
  
  # Check internet connection
  ping huggingface.co
  ```
  
- **Cache issues**:
  ```bash
  # Clear HuggingFace cache completely
  rm -rf ~/.cache/huggingface/hub/
  
  # Or clear just the model
  rm -rf ~/.cache/huggingface/hub/models--sentence-transformers--all-MiniLM-L6-v2/
  ```

- **Disk space**:
  - Model is ~90MB (model.safetensors) + tokenizer files
  - Check available space in cache directory
  - Consider moving cache: `export HF_HOME=/path/to/large/disk`

- **Permission issues**:
  ```bash
  # Fix cache permissions
  chmod -R 755 ~/.cache/huggingface/
  chown -R $USER:$USER ~/.cache/huggingface/
  ```

### Session Errors
**Problem**: Database and session-related issues

**Common Error Messages**:
- `Error connecting to <db_path>` (from diesel SQLite connection)
- `Error saving new Conversation!` (database constraint violation)
- `Error saving new Message!` (database insert failure)
- `Failed to load vector store, creating new one: <serde_yaml_error>` (vector store corruption)
- `No more memories to remove, but still over token limit` (brain token budget issues)
- `Error creating client: <config_error>` (from establish_connection panic)

**Solutions**:
- **Database connection issues**:
  ```bash
  # Check database file permissions
  ls -la ~/Library/Application\ Support/com.awful-sec.aj/aj.db  # macOS
  ls -la ~/.config/aj/aj.db  # Linux
  
  # Test database creation
  sqlite3 test.db "CREATE TABLE test (id INTEGER);" && rm test.db
  ```

- **Session/Vector Store corruption**:
  ```bash
  # Remove corrupted vector store files
  find ~/.config/aj/ -name "*_vector_store.yaml" -delete
  find ~/.config/aj/ -name "*_hnsw_index.bin" -delete
  
  # Recreate session
  aj reset
  ```

- **Token budget issues**:
  ```yaml
  # In config.yaml, increase brain token budget
  context_max_tokens: 8192  # Increase from default
  assistant_minimum_context_tokens: 2048  # Reserve more for response
  ```

- **Permission fixes**:
  ```bash
  # Fix config directory permissions
  chmod 755 ~/.config/aj/  # Linux
  chmod 755 ~/Library/Application\ Support/com.awful-sec.aj/  # macOS
  ```

### API Connection Issues
**Problem**: `Failed to connect to API server`

**Solutions**:
- Verify API base URL includes port number
- Check if server is running: `curl http://localhost:1234/v1/models`
- Test API key validity
- Check firewall settings
- Ensure TLS/SSL settings are correct for HTTPS endpoints

### RAG Performance Issues
**Problem**: Slow RAG queries or poor results

**Solutions**:
- Reduce `--rag-top-k` to 3-5 chunks
- Use smaller document files (<10MB)
- Clear RAG cache:
  ```bash
  # macOS/Linux
  rm -rf ~/Library/Application\ Support/com.awful-sec.aj/rag_cache/
  
  # Linux
  rm -rf ~/.config/aj/rag_cache/
  
  # Windows
  rmdir /s "%APPDATA%\com.awful-sec\aj\rag_cache"
  ```
- Use more specific queries

### Memory Issues
**Problem**: `Out of memory` or slow responses

**Solutions**:
- Reduce `context_max_tokens` in config
- Use smaller models for large documents
- Close other applications
- Increase swap space if applicable

### Pretty Printing Issues
**Problem**: Garbled output or no colors

**Solutions**:
- Ensure terminal supports ANSI colors
- Try without `--pretty` flag
- Check terminal encoding (UTF-8)
- Use different terminal emulator

## 🛠️ Debug Mode

Enable verbose logging to diagnose issues:

```bash
RUST_LOG=debug aj ask "test question"
```

This will show detailed information about:
- Model loading
- API requests
- Embedding computation
- Cache operations

## 📞 Getting Help

If you encounter issues not covered here:

1. **Check existing issues**: [GitHub Issues](https://github.com/graves/awful_aj/issues)
2. **Create new issue**: Include:
   - OS and version
   - `aj --version` output
   - Error message (full)
   - `config.yaml` (sanitized)
   - Steps to reproduce

3. **Community support**: Check discussions for common solutions

## 🔍 Diagnostic Commands

### Health Check
```bash
# Test basic functionality
aj ask "test" --one-shot

# Test embeddings
aj ask -r README.md "test" --one-shot

# Test session
aj interactive -s test-session
```

### Configuration Validation
```bash
# Check config syntax
aj config --validate

# Show current config
aj config --show
```

## 💡 Pro Tips

- **Always test with `--one-shot`** when troubleshooting
- **Clear caches** before reporting embedding issues
- **Check logs** with `RUST_LOG=debug` for detailed info
- **Verify paths** match your OS conventions
- **Test incrementally**: Start simple, add complexity gradually
