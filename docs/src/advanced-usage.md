# Advanced Usage 🚀

This guide covers advanced workflows and complex use cases for `aj`.

## 🔗 Complex Workflows

### Multi-Document RAG Analysis

Analyze multiple documents with semantic search:

```bash
# Load technical documentation for analysis
aj interactive -r "README.md,API.md,CHANGELOG.md" -k 10 -p

# Ask specific questions across the corpus
> How does the authentication system work?
> What are the breaking changes in v0.4.0?
> What performance optimizations were made?
```

### Session-Based Development Work

Maintain context across multiple sessions:

```bash
# Start a development session
aj interactive -s dev-project -r "src/,tests/" -p

# Later, continue the same session
aj interactive -s dev-project

# Switch to different context
aj interactive -s documentation-review -r "docs/" -k 5
```

### Batch Processing Pipeline

Use `aj` in scripts for document processing:

```bash
#!/bin/bash
# Process multiple files with RAG
for file in docs/*.md; do
  echo "Processing: $file"
  aj ask -r "$file" "Summarize key points and identify action items" -o
done

# Generate consolidated report
aj ask -r "summaries/" "Create a unified status report from all summaries" -o
```

## 🔧 Configuration Mastery

### Environment-Specific Configs

Create different configs for different environments:

```yaml
# ~/.config/aj/development.yaml
api_base: "http://localhost:1234/v1"
model: "jade_qwen3_4b_mlx"
context_max_tokens: 16384
should_stream: true

# ~/.config/aj/production.yaml  
api_base: "https://api.production.com/v1"
model: "gpt-4"
context_max_tokens: 8192
should_stream: false
```

Switch between environments:

```bash
# Use development config
aj --config ~/.config/aj/development.yaml ask "test"

# Use production config
aj --config ~/.config/aj/production.yaml ask "deploy status?"
```

### Advanced Template Engineering

Create sophisticated prompt templates:

```yaml
# templates/code-review.yaml
system_prompt: |
  You are a senior software engineer conducting code review.
  Focus on:
  - Security vulnerabilities
  - Performance issues  
  - Code maintainability
  - Best practices adherence
  
  Provide specific, actionable feedback with code examples.
  
response_format: "structured_json"

pre_user_message_content: |
  Please review the following code:
  
  --- CODE START ---
  {{code}}
  --- CODE END ---
  
  Provide feedback in JSON format:
  {
    "security_issues": [],
    "performance_issues": [],
    "maintainability_issues": [],
    "suggestions": [],
    "overall_score": 1-10
  }

post_user_message_content: |
  Please explain your reasoning for each issue found.

messages: []
```

Use advanced templates:

```bash
# Use code review template
aj interactive -t code-review -s security-audit

# Combine with RAG
aj interactive -t code-review -r "src/security/,docs/security.md" -p
```

### Custom Embedding Models

Use different embedding models for specific domains:

```yaml
# ~/.config/aj/config.yaml
embedding_model: "all-MiniLM-L6-v2"  # Default general purpose
# For code-specific embeddings
code_embedding_model: "codebert-base"  
# For medical documents
medical_embedding_model: "biobert-base"
# For legal documents
legal_embedding_model: "legal-bert-base"
```

## ⚡ Performance Optimization

### Memory Management

Optimize token usage for large documents:

```bash
# Process large documents in chunks
aj ask -r large_document.txt -k 3 "Summarize the first section"

# Use smaller context for follow-up questions
aj ask --context-tokens 2048 "What were the key points?"

# Clear memory when switching topics
aj reset
aj interactive -s new-topic
```

### Caching Strategy

Optimize RAG cache performance:

```bash
# Warm up cache with frequently accessed docs
aj ask -r "api.md,user-guide.md" -k 10 "How does authentication work?"

# Cache is now warm for subsequent queries
aj ask -r "api.md" "What are the rate limits?"
```

### Batch Operations

Process multiple queries efficiently:

```bash
# Create a session for batch work
aj interactive -s batch-analysis

# Load all documents once
aj interactive -s batch-analysis -r "docs/,specs/" -p

# Multiple questions in same session
> What are the API endpoints?
> How is error handling implemented?
> What are the performance characteristics?
```

## 🔍 Integration Examples

### CI/CD Pipeline Integration

```bash
#!/bin/bash
# CI script for automated documentation updates
set -e

# Generate release notes
RELEASE_NOTES=$(aj ask -r "CHANGELOG.md" "Generate release notes for version $VERSION" -o)

# Update API documentation  
API_DOCS=$(aj ask -r "src/api.rs" "Generate API documentation in OpenAPI format" -o)

# Create pull request template
PR_TEMPLATE=$(aj ask -t pr-template "Generate PR description for changes" -o)

echo "Release notes: $RELEASE_NOTES"
echo "API docs: $API_DOCS"
echo "PR template: $PR_TEMPLATE"
```

### Git Hook Integration

```bash
# ~/.git/hooks/pre-commit
#!/bin/bash

# Run code review before commit
if git diff --cached --name-only | grep -E '\.(rs|js|ts)$' > /dev/null; then
  aj ask -r "$(git diff --cached)" "Review staged changes for issues" -o
fi

# Generate commit message
aj ask -t commit-template "Generate commit message from changes" -o
```

### IDE Integration

#### Vim Integration

```vimscript
" Add to ~/.vimrc
nnoremap <leader>aj :!aj ask -r "%" -k 5 "Explain this code"<CR>
nnoremap <leader>ac :!aj interactive -s vim-session -r "%" -p<CR>
```

#### VS Code Integration

```json
// .vscode/tasks.json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Ask AJ about current file",
      "type": "shell", 
      "command": "aj ask -r '${file}' -k 5 'Explain this function'",
      "group": {
        "kind": "build",
        "isDefault": true
      }
    },
    {
      "label": "Start AJ interactive with project context",
      "type": "shell",
      "command": "aj interactive -s vscode-session -r '${workspaceFolder}' -p"
    }
  ]
}
```

## 🧪 Advanced Debugging

### Verbose Mode

Enable detailed logging for troubleshooting:

```bash
# Debug embedding issues
RUST_LOG=debug aj ask -r document.txt "test" -o

# Debug session persistence  
RUST_LOG=debug aj interactive -s debug-session

# Debug API calls
RUST_LOG=aj::api=trace aj ask "test question"
```

### Performance Profiling

Monitor resource usage:

```bash
# Profile memory usage
/usr/bin/time -v aj ask -r large_corpus/ "Summarize performance characteristics"

# Profile embedding computation
RUST_LOG=aj::embeddings=trace aj ask -r codebase/ "Analyze code structure"
```

### Database Inspection

Examine session storage:

```bash
# View session schema
sqlite3 ~/.config/aj/aj.db ".schema"

# Check memory usage
sqlite3 ~/.config/aj/aj.db "SELECT COUNT(*) FROM messages;"

# View recent sessions
sqlite3 ~/.config/aj/aj.db "SELECT DISTINCT session_name FROM sessions ORDER BY updated_at DESC LIMIT 10;"
```

## 💡 Pro Tips

### Workflow Optimization
- **Use named sessions** for complex projects to maintain context
- **Batch RAG loading** for multiple documents to avoid repeated embedding
- **Template inheritance** - create base templates and extend for specific use cases
- **Environment separation** - use different configs for dev/staging/production

### Performance Tips
- **Monitor memory usage** with `RUST_LOG=debug` for optimization opportunities
- **Use appropriate context sizes** - smaller for focused questions, larger for analysis
- **Leverage caching** - RAG cache persists across sessions

### Integration Best Practices
- **Script common workflows** to avoid repetitive typing
- **Use in CI/CD pipelines** for automated documentation and code review
- **Combine with other tools** - `aj` works great with `git`, `make`, etc.

## 🎯 Advanced Use Cases

### 1. Large Codebase Analysis
```bash
# Analyze entire project structure
aj interactive -s code-analysis -r "src/,tests/,docs/" -k 15 -p

# Deep dive into specific areas
> What are the security patterns used?
> How is error handling structured?
> What are the performance bottlenecks?
```

### 2. Documentation Generation Pipeline
```bash
# Generate comprehensive documentation
aj ask -r "src/,specs/,README.md" "Generate complete API documentation" -t docs-template

# Create user guides
aj ask -r "docs/user-guide.md" "Create tutorial for new users" -t tutorial-template

# Update changelog
aj ask -r "CHANGELOG.md" "Update changelog with latest changes" -t changelog-template
```

### 3. Research Assistant Workflow
```bash
# Load research papers
aj interactive -s research-session -r "papers/,articles/" -p

# Synthesize findings
> What are the key findings across all papers?
> What are the common themes and patterns?
> What are the research gaps identified?

# Generate literature review
aj ask -t lit-review-template "Create comprehensive literature review"
```

This advanced usage guide enables you to leverage `aj`'s full capabilities for complex workflows and integration scenarios.
