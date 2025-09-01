# Template Examples 🎭

Templates are at the heart of how aj guides conversations. They define who the assistant is, how the dialogue begins, and (optionally) what structure the responses must take.

A template is a simple YAML file stored under:
- macOS: `~/Library/Application Support/com.awful-sec.aj/templates/`
- Linux: `~/.config/aj/templates/`
- Windows: `%APPDATA%\awful-sec\aj\templates\`

Each template can be as minimal or as rich as you want—ranging from a single system prompt to a complex orchestration of JSON schemas and seeded conversation turns.

## 🧩 Anatomy of a Template

A template YAML file typically includes:
- `system_prompt`: The assistant’s role and global behavior.
- `messages`: Preloaded conversation history (user and assistant turns) to “frame” new queries.
- `response_format` (optional): A JSON schema that enforces structured outputs.
- `pre_user_message_content` / `post_user_message_content` (optional): Strings prepended/appended to every new user query before it is sent.

## 🎨 Example 1: Minimal Q&A
```yaml
system_prompt: You are Jade, a concise technical assistant.
messages:
  - role: user
    content: How do I list files in Rust?
  - role: assistant
    content: |-
      You can use `std::fs::read_dir`:

      ```rust
      for entry in std::fs::read_dir(".")? {
          let entry = entry?;
          println!("{:?}", entry.path());
      }
      ```
```
Now every query you make in this template context nudges the LLM to answer short and code-focused.

## 📐 Example 2: Structured JSON Output

Templates can preload conversation history to create guided behavior. For example, a "tutor" template might look like this:
```yaml
system_prompt: You are Qwen, a professor.
messages:
  - role: user
    content: |-
      Summarize this passage into three exam questions:
      Blah blah blah, you can multiple the numbers to make them escape the parenthesis which actually means something deeper.
      Some numbers just don't make no damn sense, like why they call it pie anyway. Get rooted noob.
  - role: assistant
    content: |-
      {
        "finalExamQuestion1": "Explain the distributive property with an example.",
        "finalExamQuestion2": "Compare rational and irrational numbers.",
        "finalExamQuestion3": "Simplify an expression using the laws of exponents."
      }
response_format:
  name: final_exam_question_generator
  strict: true
  description: Always return 3 exam questions.
  schema:
    type: object
    properties:
      finalExamQuestion1: { type: string }
      finalExamQuestion2: { type: string }
      finalExamQuestion3: { type: string }
    required: [finalExamQuestion1, finalExamQuestion2, finalExamQuestion3]
```
By seeding a user prompt and assistant's JSON reply, you lock in both behavior and format.

## 📖 Example 3: Literary Tutor with Context
Templates can define strict output formats using the `response_format` field.

For instance, here’s a template that enforces exam-style structured questions:
```yaml
system_prompt: You are Jade, a world literature tutor.
messages:
  - role: user
    content: Who are the Pandavas in the Gita?
  - role: assistant
    content: |-
      The Pandavas are the five sons of King Pandu:
      - **Yudhishthira** (justice and truth)
      - **Bhima** (strength)
      - **Arjuna** (archery and discipline)
      - **Nakula** and **Sahadeva** (the twins, beauty and wisdom)

      They are cousins to the Kauravas, and their conflict drives the central drama.
      ```
      Pandu → Yudhishthira, Bhima, Arjuna, Nakula, Sahadeva
      Dhritarashtra → 100 Kauravas (incl. Duryodhana)
      ```
      This family tree is crucial for understanding loyalties and rivalries in the Gita.
```
This ensures the LLM always explains relationships clearly, often including family trees.

## 🛠️ Example 4: Refactoring Code
```yaml
system_prompt: You are Jade, an expert Rust refactoring assistant.
messages:
  - role: user
    content: Refactor this code to use iterators.
  - role: assistant
    content: |-
      Sure! Here’s the refactored version:

      ```rust
      let nums = vec![1, 2, 3, 4, 5];
      let squares: Vec<_> = nums.iter().map(|x| x * x).collect();
      println!("{:?}", squares);
      ```
      This avoids indexing and uses iterator combinators idiomatically.
```
Notice how the assistant reply not only refactors but also explains why. Every future query in this template will follow that pattern.


## ✨ Practical Tips
- Always pair user + assistant in seeded messages if you want to strongly guide style.
- Use `response_format` for machine-readable guarantees (JSON, tables, etc.).
- Use `pre_user_message_content` / `post_user_message_content` for lightweight “wrapping” (like always appending `/nothink`).
- Keep multiple templates—switch roles instantly (`reading_buddy.yaml`, `exam_generator.yaml`, `code_refactor.yaml`).

## 🚀 Where to Go From Here
- Try starting with `simple_question.yaml`.
- Copy it into `refactor_rust.yaml` or `book_knowledge_synthesizer.yaml` to see how far you can push complexity.
- Remember: templates are just YAML. They can be versioned, shared, and tweaked freely.
