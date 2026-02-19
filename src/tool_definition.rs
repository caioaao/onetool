//! Tool definition metadata for LLM integration.
//!
//! This module provides structured metadata for the Lua REPL tool,
//! enabling integration with various LLM clients (OpenAI, Anthropic, Google, etc.).

/// Tool name identifier for LLM clients.
pub const NAME: &str = "lua_repl";

/// Comprehensive tool description for LLM context.
pub const DESCRIPTION: &str = r#"Execute Lua code in a long-lived sandboxed REPL environment.

**Capabilities:**
- Expression evaluation with return values
- print() output capture (appears in tool response)
- Persistent state between executions (variables, functions, tables)
- Safe operations: string, table, math, utf8, os.time, os.date
- Documentation: available via global `docs` variable

**Restrictions:**
- No file I/O or network access
- No OS command execution
- No code loading (require, load, loadfile)
- No dangerous metatable operations

**Environment:**
- Sandboxed Lua 5.4

**Input:**
- source_code: Lua code to execute

**Output:**
- result: Expression evaluation result (array of strings) or error message
- output: Lines from print() calls (array of strings)

**Example Usage:**
```lua
-- Calculate and print
x = 10
y = 20
print("Sum:", x + y)
return x + y
-- Result: ["30"]
-- Output: ["Sum:\t10\t20\n"]
```
"#;

/// Parameter name for the source code input.
pub const PARAM_SOURCE_CODE: &str = "source_code";

/// Description of the source_code parameter.
pub const PARAM_SOURCE_CODE_DESC: &str =
    "Lua source code to execute in the sandboxed REPL environment";

/// Returns JSON Schema for the tool's input parameters.
///
/// Compatible with OpenAI function calling, Google Gemini, and other
/// JSON Schema-based tool interfaces.
///
/// Requires the `json_schema` feature to be enabled.
#[cfg(feature = "json_schema")]
pub fn json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            PARAM_SOURCE_CODE: {
                "type": "string",
                "description": PARAM_SOURCE_CODE_DESC
            }
        },
        "required": [PARAM_SOURCE_CODE]
    })
}
