# onetool

**The last LLM tool you'll need.**

## The Problem

LLM agents typically get dozens of specialized tools:
- A calculator for arithmetic
- A date formatter for timestamps
- A string manipulator for text operations
- A JSON parser, a base64 encoder, a hash generator...

Each tool requires API design, documentation, and testing. Tools don't compose well. And you're always limited by what tools you thought to create.

## The Solution

**onetool provides one universal computation tool** powered by a sandboxed Lua runtime.

Instead of hunting for the right tool, your LLM can solve problems programmatically. State persists between calls for multi-step reasoning. It's safe by design with comprehensive sandboxing. And it integrates seamlessly with major LLM libraries.

## Quick Start: LLM Integration

```rust
use onetool::Repl;
use genai::chat::{ChatRequest, ChatMessage, ToolResponse};
use serde_json::json;

// 1. Create the REPL
let repl = Repl::new()?;

// 2. Get the tool definition (compatible with OpenAI, Google, etc.)
let tool = onetool::tool_definition::genai_tool();

// 3. Add to your chat request
let chat_req = ChatRequest::new(vec![
    ChatMessage::user("What's the sum of the 10 first prime numbers?")
]).with_tools(vec![tool]);

// 4. Get LLM response with tool calls
let chat_res = client.exec_chat(MODEL, chat_req.clone(), None).await?;
let tool_calls = chat_res.into_tool_calls();

// 5. Execute the code
let source_code = &tool_calls[0].fn_arguments["source_code"];
let response = repl.eval(source_code)?;

// 6. Send results back to LLM
let tool_response = ToolResponse::new(
    tool_calls[0].call_id.clone(),
    json!({
        "output": response.output.join("\n"),
        "result": match response.result {
            Ok(result) => result.join("\n"),
            Err(err) => format!("error: {}", err),
        }
    }).to_string()
);

// 7. Get final answer
let chat_req = chat_req
    .append_message(tool_calls)
    .append_message(tool_response);
let final_response = client.exec_chat(MODEL, chat_req, None).await?;
```

## Real Example: What Can It Do?

Here's an actual interaction from the included example:

```
User: "What's the sum of the 10 first prime numbers?"

LLM calls lua_repl with:
{
  "source_code": "
    local primes = {}
    local num = 2
    while #primes < 10 do
      local is_prime = true
      for i = 2, math.sqrt(num) do
        if num % i == 0 then
          is_prime = false
          break
        end
      end
      if is_prime then
        table.insert(primes, num)
      end
      num = num + 1
    end

    local sum = 0
    for _, p in ipairs(primes) do
      sum = sum + p
    end
    return sum
  "
}

Response: {
  "result": "129",
  "output": ""
}

LLM: "The sum of the first 10 prime numbers is 129."
```

The LLM wrote a complete algorithm, executed it safely, and got the answer - all without needing a specialized "prime number calculator" tool.

## Tool Definition System

onetool includes a complete tool definition system for LLM integration:

```rust
use onetool::tool_definition;

// Tool metadata
tool_definition::NAME              // "lua_repl"
tool_definition::DESCRIPTION       // Full description for LLM context
tool_definition::PARAM_SOURCE_CODE // "source_code"

// JSON Schema (for OpenAI, Google, Anthropic)
let schema = tool_definition::json_schema();

// genai integration (requires "genai" feature)
let tool = tool_definition::genai_tool();
```

**Compatible with:**
- OpenAI function calling
- Google Gemini function calling
- Anthropic tool use
- Any JSON Schema-based tool system

## Security Model

### Safe by Design

- **Sandboxed Lua 5.4 runtime** - Dangerous operations blocked at the language level

### What's Available

- String manipulation (`string.*`)
- Table operations (`table.*`)
- Math functions (`math.*`)
- UTF-8 support (`utf8.*`)
- Safe OS functions (`os.time`, `os.date`)
- All Lua control flow and data structures

### What's Blocked

- File I/O (`io`, `file`)
- Network access
- Code loading (`require`, `dofile`, `load*`)
- OS commands (`os.execute`, `os.getenv`, etc.)
- Metatable manipulation
- Coroutines
- Garbage collection control

## Key Features

**For LLM Integration:**
- Universal computation tool (replaces dozens of specialized tools)
- Built-in tool definitions (OpenAI, Google, Anthropic compatible)
- JSON Schema generation
- Comprehensive documentation in tool description

**For Developers:**
- Drop-in integration with genai library
- Separate `print()` output from return values
- Clear error messages
- Type-safe Rust API via mlua

**For LLM Agents:**
- Persistent state between calls (variables, functions, tables)
- Runtime introspection via `docs` global
- Can solve multi-step problems programmatically
- Self-documenting environment

## Installation

**Basic setup:**
```toml
[dependencies]
onetool = "0.0.1-alpha.3"
tokio = { version = "1", features = ["full"] }
```

**With genai integration:**
```toml
[dependencies]
onetool = { version = "0.0.1-alpha.3", features = ["genai"] }
genai = "0.5"
```

**With JSON Schema only:**
```toml
[dependencies]
onetool = { version = "0.0.1-alpha.3", features = ["json_schema"] }
```

**Note:** Currently in alpha - API may change.

## Complete Example

**Run the full LLM integration example:**
```bash
# Set your API key (OpenAI, Google, etc.)
export OPENAI_API_KEY=your_key_here

# Run the example
cargo run --features genai --example genai-basic
```

This demonstrates:
- Creating the sandboxed REPL
- Registering the tool with an LLM client
- LLM generating Lua code to solve a problem
- Executing the code safely
- Returning results to the LLM
- LLM synthesizing the final answer

**See it in action:**

The example asks the LLM to calculate the sum of the first 10 prime numbers. The LLM:
1. Writes Lua code to find primes
2. Writes code to sum them
3. Executes via onetool
4. Receives the answer (129)
5. Responds to the user

See the full example source: [`examples/genai-basic.rs`](examples/genai-basic.rs)

## Other Examples

**Interactive REPL (for testing):**
```bash
cargo run --example lua-repl
```

This lets you interact with the sandboxed environment directly to test Lua code and understand what the LLM sees.

## API Overview


Full API documentation available at [docs.rs/onetool](https://docs.rs/onetool).

## Why Lua?

- **Lightweight**: Small runtime, fast startup
- **Embeddable**: Designed from the ground up to be embedded in host applications
- **Simple**: Easy for LLMs to generate correct code
- **Powerful**: Full programming language, not a domain-specific language
- **Safe**: Straightforward to sandbox effectively

## Use Cases

**Perfect for:**
- LLM agents that need computation capabilities
- AI assistants with multi-step reasoning
- Applications requiring safe user-generated code execution

## Project Status

- **Version**: 0.0.1-alpha.3
- **Stability**: Alpha - API may change, but core concept is stable
- **Production Ready**: Not yet - use at your own risk

## Development

**Building:**
```bash
cargo build
cargo test
cargo doc --open
```

**Nix Support:**
```bash
nix develop  # Dev shell with Rust, cargo-watch, rust-analyzer
```

**Running Examples:**
```bash
cargo run --features genai --example genai-basic  # Full LLM integration
cargo run --example lua-repl     # Interactive testing
```

## Architecture

For implementation details, see:
- [`src/runtime/mod.rs`](src/runtime/mod.rs) - Lua runtime definition
- [`src/runtime/docs.rs`](src/runtime/docs.rs) - Runtime documentation implementation
- [`src/runtime/sandbox.rs`](src/runtime/sandbox.rs) - Sandboxing implementation
- [`src/tool_definition.rs`](src/tool_definition.rs) - Tool integration system

**Key patterns:**
- Nil-based sandboxing (simple, effective)
- Output capture via mpsc channels
- Persistent Lua state across invocations
- Runtime documentation system

## License & Contributing

**License:** MIT - Copyright 2026 Caio Augusto Araujo Oliveira

**Contributing:**
- Early stage project - feedback welcome!
- Issues and PRs appreciated

---

Built with [mlua](https://github.com/mlua-rs/mlua) and [genai](https://github.com/jeremychone/rust-genai).
