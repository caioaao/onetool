# onetool

**Sandboxed Lua runtime for LLM tool use.**

[API Documentation](https://docs.rs/onetool)

## The Problem

LLM agents typically need dozens of specialized tools (calculator, date formatter, string manipulator, JSON parser, base64 encoder, hash generator, etc.). Each tool requires a round-trip to the LLM provider, and you pay for every token exchanged. Tools don't compose well, and you're always limited by what you thought to create.

## The Solution

**onetool provides a sandboxed Lua REPL** that LLMs can use as a single tool.

LLMs are already trained on programming languages. By giving them code execution instead of specialized tools, you reduce token costs (one tool call instead of many) while increasing flexibility. State persists between calls for multi-step reasoning. It's safe by design with comprehensive sandboxing.

## Installation

**Basic REPL only** (no LLM framework):
```toml
[dependencies]
onetool = "0.0.1-alpha.10"
```

**With a framework adapter:**
```toml
# Pick one (or more):
onetool = { version = "0.0.1-alpha.10", features = ["genai"] }
onetool = { version = "0.0.1-alpha.10", features = ["mistralrs"] }
onetool = { version = "0.0.1-alpha.10", features = ["rig"] }
onetool = { version = "0.0.1-alpha.10", features = ["aisdk"] }
onetool = { version = "0.0.1-alpha.10", features = ["mcp"] }
```

**Feature flags:**

| Feature | Description |
|---------|-------------|
| `genai` | [genai](https://github.com/jeremychone/rust-genai) adapter |
| `mistralrs` | [mistral.rs](https://github.com/EricLBuehler/mistral.rs) adapter |
| `rig` | [rig-core](https://github.com/0xPlaygrounds/rig) `Tool` implementation |
| `aisdk` | [aisdk](https://github.com/lazy-hq/aisdk) integration |
| `mcp` | [MCP](https://modelcontextprotocol.io/) server via rmcp |
| `json_schema` | JSON Schema generation (included by all above) |

**Note:** Currently in alpha - API may change.

## Quick Start

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

## Real Example

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

## Why Lua?

These were the criteria for choosing the execution language:

- **Interpreted**: We can't depend on a compile-eval loop
- **Easy to embed**: The runtime needs to live inside the host application
- **Easy to sandbox**: Giving too much power to an LLM can be dangerous
- **Simple and expressive**: LLMs need to write small, correct snippets
- **Strong standard library**: Especially for string manipulation
- **Mature and well-known**: Editor plugins, documentation, familiarity

Lua checks all these boxes. It's widespread enough (neovim config language, game scripting) that LLMs are well-trained on it.

## Running the Examples

```bash
# Interactive REPL (no API key required)
cargo run --example lua-repl

# Custom Rust functions in the runtime
cargo run --example custom-functions

# Interactive notebook demo (requires API key)
cargo run --features notebook_demo --example notebook-demo # DEEPSEEK_API_KEY

# LLM framework examples (require API keys where noted)
cargo run --features genai --example genai-basic        # DEEPSEEK_API_KEY
cargo run --features mistralrs --example mistralrs-basic # local inference, no key
cargo run --features rig --example rig-basic             # DEEPSEEK_API_KEY
cargo run --features aisdk --example aisdk-basic         # DEEPSEEK_API_KEY
cargo run --features mcp --example mcp-basic             # MCP stdio server
```

## Project Status

**This is still a toy project.** Use with care - everything may break, and I might decide to change everything tomorrow.

- **Version**: 0.0.1-alpha.10
- **API Stability**: Expect breaking changes
- **Production Ready**: No

The core concept is stable (sandboxed Lua REPL for LLMs), but the implementation and API surface are experimental.

## License & Contributing

**License:** MIT - Copyright 2026 Caio Augusto Araujo Oliveira

**Contributing:**
- Early stage project - feedback welcome!
- Issues and PRs appreciated

---

Built with [mlua](https://github.com/mlua-rs/mlua).
