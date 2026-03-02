//! Tool definition metadata for LLM integration.
//!
//! This module provides structured metadata for the Lua REPL tool,
//! enabling integration with various LLM clients (OpenAI, Anthropic, Google, etc.).

/// Tool name identifier for LLM clients.
pub const NAME: &str = "lua_repl";

/// Comprehensive tool description for LLM context.
pub const DESCRIPTION: &str = r#"Execute Lua code in a long-lived sandboxed REPL environment.

**IMPORTANT:** Provide ONLY valid Lua code as input. Do NOT wrap code in markdown blocks.

**Custom Functions — Check `docs` First:**
The application may register domain-specific functions (HTTP, database, translation, etc.) that extend this REPL beyond standard Lua. These are documented in the `docs` global table. ALWAYS check it when a task involves capabilities beyond standard Lua:
  -- List all available custom functions
  for k, v in pairs(docs) do print(k, v) end
  -- Check a specific function
  print(docs["function_name"])

**Environment:**
- Sandboxed Lua 5.4 with persistent state between calls
- Global variables (assigned without `local`) persist across calls
- Local variables (`local x = ...`) are lost between calls
- Custom global variables may be injected by the application

**Always Available:**
- Full string library (string.sub, string.find, string.match, string.gsub, string.gmatch, etc.)
- Full table library (table.insert, table.concat, table.remove, table.sort, etc.)
- Full math library (math.min, math.max, math.floor, math.ceil, math.abs, math.sqrt, etc.)
- UTF-8 support (utf8.*)
- Core functions: type(), tonumber(), tostring(), pairs(), ipairs(), select(), assert(), error(), pcall(), xpcall()
- Output: print() (captured in tool response)
- Time: os.time(), os.date()

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

**Pattern Matching (Lua patterns, NOT regex):**
Lua uses its own pattern syntax. Common regex features that DO NOT work:
  \d, \w, \s → use %d, %a, %s instead
  [a-z] → use %l (lowercase) or %a (letter)
  \b, |, {n} → not available
Character classes: %d (digit), %a (letter), %l (lower), %u (upper), %s (space), %p (punctuation), %w (alphanumeric), %c (control)
Quantifiers: + (1+), * (0+), - (0+ lazy), ? (0 or 1)
Captures: use () — string.match("hello 42", "(%d+)") returns "42"
Iteration: for word in string.gmatch(text, "%S+") do print(word) end

**Usage:**
- Use print() to output intermediate values for debugging
- Use return to provide final results
- Prefer iterative algorithms — naive recursion can be extremely slow for large inputs
- Search large strings with string.match/string.find — don't print them (output truncation may hide data)

**Example:**
x = 10
y = 20
print("Computing sum...")
return x + y

**Output Truncation:**
- Output and result are automatically truncated to 50,000 characters by default
- Truncated values end with "...\n(output truncated)"
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
