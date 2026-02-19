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
- **[mistralrs](#mistralrs-adapter)** - `LuaRepl::new(&repl)` with `definition()` and `call()` methods
- **[rig](#rig-adapter)** - Implements `Tool` trait (requires `set_repl()` first)
- **[aisdk](#aisdk-adapter)** - Uses `#[tool]` macro (requires `set_repl()` first)

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
- `LuaRepl::new(&repl)` - Creates the adapter
- `.definition()` - Returns `mistralrs::Tool` for registration
- `.call(&tool_call)` - Executes tool call and returns result string

**Example:**

```rust
use onetool::{Repl, mistralrs::LuaRepl};

let repl = Repl::new()?;
let lua_repl = LuaRepl::new(&repl);

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

**Important:** You must call `onetool::rig::set_repl()` before creating the tool, as rig requires tools to be `Sync`.

**Key Methods:**
- `onetool::rig::set_repl(repl)` - Initialize global REPL (call once)
- `LuaRepl::new()` - Creates the tool (implements `Tool` trait)

**Example:**

```rust
use onetool::{Repl, rig::{set_repl, LuaRepl}};

let repl = Repl::new()?;
set_repl(repl);  // Must be called first!

let lua_tool = LuaRepl::new();

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

The aisdk adapter uses the `#[tool]` macro from [aisdk](https://github.com/lazy-hq/aisdk).

**Important:** You must call `onetool::aisdk::set_repl()` before using the tool, as the macro generates a function-based tool.

**Key Functions:**
- `onetool::aisdk::set_repl(repl)` - Initialize global REPL (call once)
- `onetool::aisdk::lua_repl()` - Returns the tool function

**Example:**

```rust
use onetool::{Repl, aisdk};

let repl = Repl::new()?;
aisdk::set_repl(repl);  // Must be called first!

// Use with aisdk
let result = LanguageModelRequest::builder()
    .model(OpenAI::gpt_4o())
    .prompt("Calculate something")
    .with_tool(aisdk::lua_repl())
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
- Universal computation tool (replaces dozens of specialized tools)
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

- **Version**: 0.0.1-alpha.4
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
