# onetool

**Sandboxed Lua runtime for LLM tool use.**

## The Problem

LLM agents typically need dozens of specialized tools (calculator, date formatter, string manipulator, JSON parser, base64 encoder, hash generator, etc.). This creates two problems:

1. **Token costs add up**: Each tool call requires a round-trip to the LLM provider. Complex tasks need multiple calls, and you pay for every token exchanged.
2. **Tool proliferation**: Each new tool requires API design, documentation, and testing. Tools don't compose well. And you're always limited by what you thought to create.

What if the LLM could solve problems programmatically instead of making multiple tool calls? We already have the perfect interface for that: programming languages.

## The Solution

**onetool provides a sandboxed Lua REPL** that LLMs can use as a tool.

LLMs are already trained on programming languages. By giving them code execution instead of specialized tools, you reduce token costs (one tool call instead of many) while increasing flexibility. State persists between calls for multi-step reasoning. It's safe by design with comprehensive sandboxing.

**Prior art:** Cloudflare and Anthropic have explored similar approaches with their [Code Mode](https://blog.cloudflare.com/code-mode/) and [MCP code execution](https://www.anthropic.com/engineering/code-execution-with-mcp) respectively.

## Framework Support

onetool provides adapters for popular Rust LLM frameworks:

- **[genai](https://github.com/jeremychone/rust-genai)** - Multi-provider LLM client (OpenAI, Google, Anthropic)
- **[mistral.rs](https://github.com/EricLBuehler/mistral.rs)** - Fast local model inference
- **[rig](https://github.com/0xPlaygrounds/rig)** - Modular LLM application framework
- **[aisdk](https://github.com/lazy-hq/aisdk)** - Rust port of Vercel's AI SDK

See [Framework Integration](#framework-integration) for usage details.

## Quick Start: LLM Integration

### Core REPL Usage

```rust
use onetool::Repl;

// Create the sandboxed Lua runtime
let repl = Repl::new()?;

// Execute Lua code
let response = repl.eval("return 2 + 2")?;

// Access results
println!("Result: {}", response.result.unwrap().join("\n")); // "4"
println!("Output: {}", response.output.join("\n"));          // (print() output)
```

The REPL maintains state between calls, so variables and functions persist:

```rust
repl.eval("x = 10")?;
repl.eval("y = 20")?;
let result = repl.eval("return x + y")?; // "30"
```

### LLM Framework Integration

onetool provides ready-to-use adapters for popular LLM frameworks:

- **[genai](#genai-adapter)** - `LuaRepl::new(&repl)` with `definition()` and `call()` methods
- **[mistralrs](#mistralrs-adapter)** - `LuaRepl::new(repl)` with `definition()` and `call()` methods
- **[rig](#rig-adapter)** - `LuaRepl::new(repl)` implements `Tool` trait
- **[aisdk](#aisdk-adapter)** - `LuaRepl::new(repl)` with `.tool()` method

Each adapter handles tool definition registration and execution for its framework. See [Framework Integration](#framework-integration) for detailed usage.

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

## Framework Integration

### genai Adapter

**Feature flag:** `genai`

The genai adapter provides seamless integration with the [genai](https://github.com/jeremychone/rust-genai) multi-provider LLM client.

**Key Methods:**
- `LuaRepl::new(&repl)` - Creates the adapter
- `.definition()` - Returns `genai::chat::Tool` for registration
- `.call(&tool_call)` - Executes tool call and returns `ToolResponse`

**Example:**

```rust
use onetool::{Repl, genai::LuaRepl};

let repl = Repl::new()?;
let lua_repl = LuaRepl::new(&repl);

// Register with genai client
let chat_req = genai::chat::ChatRequest::new(messages)
    .with_tools(vec![lua_repl.definition()]);

// Execute tool calls
let tool_response = lua_repl.call(&tool_calls[0]);
```

**Full example:** [`examples/genai-basic.rs`](examples/genai-basic.rs)

---

### mistralrs Adapter

**Feature flag:** `mistralrs`

The mistralrs adapter integrates with [mistral.rs](https://github.com/EricLBuehler/mistral.rs) for fast local model inference.

**Key Methods:**
- `LuaRepl::new(repl)` - Creates the adapter
- `.definition()` - Returns `mistralrs::Tool` for registration
- `.call(&tool_call)` - Executes tool call and returns result string

**Example:**

```rust
use onetool::{Repl, mistralrs::LuaRepl};

let repl = Repl::new()?;
let lua_repl = LuaRepl::new(repl);

// Register with mistralrs model
let messages = RequestBuilder::new()
    .add_message(TextMessageRole::User, "Calculate something")
    .set_tools(vec![lua_repl.definition()]);

// Execute tool calls
let result = lua_repl.call(&tool_calls[0]);
```

**Full example:** [`examples/mistralrs-basic.rs`](examples/mistralrs-basic.rs)

---

### rig Adapter

**Feature flag:** `rig`

The rig adapter implements the `Tool` trait from [rig-core](https://github.com/0xPlaygrounds/rig).

**Key Methods:**
- `LuaRepl::new(repl)` - Creates the tool (implements `Tool` trait)

**Example:**

```rust
use onetool::{Repl, rig::LuaRepl};

let repl = Repl::new()?;
let lua_tool = LuaRepl::new(repl);

// Use with rig agents
let agent = client
    .agent(model)
    .tool(lua_tool)
    .build();
```

**Full example:** [`examples/rig-basic.rs`](examples/rig-basic.rs)

---

### aisdk Adapter

**Feature flag:** `aisdk`

The aisdk adapter provides integration with [aisdk](https://github.com/lazy-hq/aisdk).

**Key Methods:**
- `LuaRepl::new(repl)` - Creates the adapter
- `.tool()` - Returns a tool function for use with aisdk

**Example:**

```rust
use onetool::{Repl, aisdk::LuaRepl};

let repl = Repl::new()?;
let lua_repl = LuaRepl::new(repl);

// Use with aisdk
let result = LanguageModelRequest::builder()
    .model(OpenAI::gpt_4o())
    .prompt("Calculate something")
    .with_tool(lua_repl.tool())
    .build()
    .generate_text()
    .await?;
```

**Full example:** [`examples/aisdk-basic.rs`](examples/aisdk-basic.rs)

## Tool Definition System

onetool includes a complete tool definition system that works with any LLM framework:

```rust
use onetool::tool_definition;

// Tool metadata
tool_definition::NAME              // "lua_repl"
tool_definition::DESCRIPTION       // Full description for LLM context
tool_definition::PARAM_SOURCE_CODE // "source_code"

// JSON Schema (framework-agnostic)
let schema = tool_definition::json_schema();
```

**Framework-specific helpers:**

```rust
// genai (requires "genai" feature)
let tool = tool_definition::genai_tool();

// For mistralrs, rig, aisdk: use the adapter's .definition() method
// See Framework Integration section above
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
- Code execution as a single tool (reduces need for specialized tools)
- Built-in tool definitions (OpenAI, Google, Anthropic compatible)
- JSON Schema generation
- Comprehensive documentation in tool description

**For Developers:**
- Drop-in integration with genai, mistralrs, rig, and aisdk libraries
- Separate `print()` output from return values
- Clear error messages
- Type-safe Rust API via mlua

**For LLM Agents:**
- Persistent state between calls (variables, functions, tables)
- Runtime introspection via `docs` global
- Can solve multi-step problems programmatically
- Self-documenting environment

## Installation

**Basic REPL only** (no LLM framework):
```toml
[dependencies]
onetool = "0.0.1-alpha.4"
```

**With genai:**
```toml
[dependencies]
onetool = { version = "0.0.1-alpha.4", features = ["genai"] }
genai = "0.5"
```

**With mistralrs:**
```toml
[dependencies]
onetool = { version = "0.0.1-alpha.4", features = ["mistralrs"] }
mistralrs = { git = "https://github.com/EricLBuehler/mistral.rs.git" }
```

**With rig:**
```toml
[dependencies]
onetool = { version = "0.0.1-alpha.4", features = ["rig"] }
rig-core = "0.3"
```

**With aisdk:**
```toml
[dependencies]
onetool = { version = "0.0.1-alpha.4", features = ["aisdk"] }
aisdk = "0.2"
```

**Feature flags:**

| Feature | Includes | Description |
|---------|----------|-------------|
| `genai` | `json_schema` | genai adapter + tool definition |
| `mistralrs` | `json_schema` | mistralrs adapter + tool definition |
| `rig` | `json_schema` | rig-core Tool implementation |
| `aisdk` | `json_schema` | aisdk #[tool] macro integration |
| `json_schema` | - | JSON Schema generation (included by all above) |

**Note:** Currently in alpha - API may change.

## Running the Examples

All examples solve the same problem (sum of first 10 primes = 129) to demonstrate consistent behavior across frameworks.

### LLM Framework Examples

**genai** (multi-provider client):
```bash
export OPENAI_API_KEY=your_key_here  # or GEMINI_API_KEY, etc.
cargo run --features genai --example genai-basic
```
Source: [`examples/genai-basic.rs`](examples/genai-basic.rs)

**mistralrs** (local inference):
```bash
cargo run --features mistralrs --example mistralrs-basic
```
Downloads and runs Phi-3.5-mini locally. No API key required.
Source: [`examples/mistralrs-basic.rs`](examples/mistralrs-basic.rs)

**rig** (modular framework):
```bash
export OPENAI_API_KEY=your_key_here
cargo run --features rig --example rig-basic
```
Source: [`examples/rig-basic.rs`](examples/rig-basic.rs)

**aisdk** (Vercel AI SDK port):
```bash
export OPENAI_API_KEY=your_key_here
cargo run --features aisdk --example aisdk-basic
```
Source: [`examples/aisdk-basic.rs`](examples/aisdk-basic.rs)

### Interactive REPL

Test the sandboxed environment directly:
```bash
cargo run --example lua-repl
```

This lets you experiment with Lua code and understand what the LLM sees. No API key required.

**custom-functions** (runtime extension):
```bash
cargo run --example custom-functions
```
Shows how to extend the runtime with custom Rust functions. Includes interactive REPL for testing.
Source: [`examples/custom-functions.rs`](examples/custom-functions.rs)

## Extending the Runtime

onetool allows you to extend the Lua runtime with custom Rust functions, enabling domain-specific capabilities for your LLM agents.

### Extension Methods

There are two approaches to adding custom functions:

#### Method 1: Post-Initialization (`with_runtime()`)

Best for adding functions after creating the REPL:

```rust
use onetool::Repl;

let repl = Repl::new()?;

// Add a custom function
repl.with_runtime(|lua| {
    let my_func = lua.create_function(|_, name: String| {
        Ok(format!("Hello, {}!", name))
    })?;
    lua.globals().set("greet", my_func)?;
    Ok(())
})?;

// Now callable from Lua
let result = repl.eval("return greet('World')")?;
```

**Use when:**
- Adding functions to an existing REPL
- Functions don't need to interact with sandboxing
- Simpler initialization flow

#### Method 2: Pre-Sandboxing (`new_with()`)

Best for complex initialization scenarios:

```rust
use onetool::{Repl, runtime};

let lua = mlua::Lua::new();

// Set up custom globals
lua.globals().set("API_KEY", "secret")?;

// Register custom functions
let fetch = lua.create_function(|_, url: String| {
    // ... implementation
    Ok("response".to_string())
})?;
lua.globals().set("fetch", fetch)?;

// Apply sandboxing AFTER custom setup
runtime::sandbox::apply(&lua)?;

let repl = Repl::new_with(lua)?;
```

**Use when:**
- Need to set up complex state before sandboxing
- Custom functions require special initialization
- Building framework adapters

### Complete Example

See [`examples/custom-functions.rs`](examples/custom-functions.rs) for a complete demonstration including:
- Multiple function patterns (simple, error handling, stateful)
- Stateful closures with Arc + Atomic
- Error propagation from Rust to Lua
- Documentation registration
- Interactive testing

### Registering Documentation

Make your custom functions discoverable via the `docs` system:

```rust
use onetool::runtime::docs::{register, LuaDoc, LuaDocTyp};

repl.with_runtime(|lua| {
    // ... create and register function ...

    // Register documentation
    register(lua, &LuaDoc {
        name: "my_function".to_string(),
        typ: LuaDocTyp::Function,
        description: "Does something useful".to_string(),
    })?;
    Ok(())
})?;
```

The LLM can then query `docs["my_function"]` at runtime to understand available functions.

## API Overview


Full API documentation available at [docs.rs/onetool](https://docs.rs/onetool).

## Why Lua?

These were the criteria for choosing the execution language:

- **Interpreted**: We can't depend on a compile-eval loop
- **Easy to embed**: The runtime needs to live inside the host application
- **Easy to sandbox**: Giving too much power to an LLM can be dangerous
- **Simple and expressive**: LLMs need to write small, correct snippets
- **Strong standard library**: Especially for string manipulation
- **Mature and well-known**: Editor plugins, documentation, familiarity

Lua checks all these boxes. It's widespread enough (neovim config language, game scripting) that LLMs are well-trained on it.

## Use Cases

- LLM agents that need computation capabilities
- AI assistants with multi-step reasoning
- Applications requiring safe user-generated code execution

## Project Status

**This is still a toy project.** Use with care - everything may break, and I might decide to change everything tomorrow.

- **Version**: 0.0.1-alpha.4
- **API Stability**: Expect breaking changes
- **Production Ready**: No

The core concept is stable (sandboxed Lua REPL for LLMs), but the implementation and API surface are experimental.

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
# Framework examples (requires API keys for genai, rig, aisdk)
cargo run --features genai --example genai-basic
cargo run --features mistralrs --example mistralrs-basic
cargo run --features rig --example rig-basic
cargo run --features aisdk --example aisdk-basic

# Interactive REPL
cargo run --example lua-repl
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

Built with [mlua](https://github.com/mlua-rs/mlua).
