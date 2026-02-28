//! Tool definition metadata for LLM integration.
//!
//! This module provides structured metadata for the Lua REPL tool,
//! enabling integration with various LLM clients (OpenAI, Anthropic, Google, etc.).

/// Tool name identifier for LLM clients.
pub const NAME: &str = "lua_repl";

/// Comprehensive tool description for LLM context.
pub const DESCRIPTION: &str = r#"Execute Lua code in a long-lived sandboxed REPL environment.

**IMPORTANT:** Provide ONLY valid Lua code as input. Do NOT wrap code in markdown blocks.

**Environment:**
- Sandboxed Lua 5.4 with persistent state between calls
- Global variables (assigned without `local`) persist across calls
- Local variables (`local x = ...`) are lost between calls
- Custom global variables may be injected by the application (check what's available!)

**Always Available:**
- Full string library (string.sub, string.find, string.match, string.gsub, string.gmatch, etc.)
- Full table library (table.insert, table.concat, table.remove, table.sort, etc.)
- Full math library (math.min, math.max, math.floor, math.ceil, math.abs, math.sqrt, etc.)
- UTF-8 support (utf8.*)
- Core functions: type(), tonumber(), tostring(), pairs(), ipairs(), select(), assert(), error(), pcall(), xpcall()
- Output: print() (captured in tool response)
- Time: os.time(), os.date()
- Docs: `docs` global contains documentation for custom functions

**Policy-Controlled (may or may not be available):**
The following operations are disabled by default but may be enabled by the application's policy. When disabled, they return nil. Probe before using:
  if io.open then
    local f = io.open("data.txt", "r")
    -- use f
  end
- File I/O: io.open, io.read, io.write, io.lines, io.close
- OS operations: os.execute, os.remove, os.rename, os.getenv, os.tmpname
- Code loading: load(), loadfile(), dofile()
- Metatable manipulation: rawset, rawget, setmetatable, getmetatable

**Never Available:**
- debug library
- coroutine library
- package/require system

**Pattern Matching:**
Lua patterns (NOT regex): `.` (any char), `%d` (digit), `%a` (letter), `%s` (space), `+` (1 or more), `*` (0 or more)
Example: string.match(text, "number is (%d+)") -- captures digits after "number is "

**Usage:**
- Use print() to output intermediate values for debugging
- Use return to provide final results
- Access any global variables that have been set by the application
- Leverage Lua for data processing - it's fast and efficient even on large strings

**Example:**
x = 10
y = 20
print("Computing sum...")
return x + y

**Output Truncation:**
- Output and result are automatically truncated to 50,000 characters by default
- Truncated values end with "...\n(output truncated)"
- This prevents extremely large outputs from overwhelming the conversation context
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
