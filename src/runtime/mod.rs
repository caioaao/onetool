//! Runtime creation and sandboxing.
//!
//! This module provides functions to create sandboxed Lua runtimes. The sandbox
//! restricts dangerous operations using policy-based access control while preserving
//! safe Lua standard library functionality.
//!
//! # Security Model
//!
//! Sandboxing is applied using a three-tier policy-based approach:
//! - **Safe** functions work without restrictions (e.g., `os.time`, `math.*`, `string.*`)
//! - **Unsafe** functions are wrapped and return `nil` on policy denial (e.g., `os.execute`, `io.open`)
//! - **Forbidden** functions are removed entirely (e.g., `debug`, `coroutine`, `package`)
//!
//! Sandboxed runtimes allow:
//! - String manipulation (string.*)
//! - Table operations (table.*)
//! - Math functions (math.*)
//! - UTF-8 support (utf8.*)
//! - Safe OS functions (os.time, os.date)
//! - Basic operations (print, ipairs, pairs, next, select, assert, error, pcall, xpcall)
//!
//! Sandboxed runtimes wrap (return nil by default):
//! - File I/O (io.*)
//! - Code loading (require, dofile, load, loadfile)
//! - OS commands (os.execute, os.getenv, os.remove, os.rename)
//! - Metatable manipulation (getmetatable, setmetatable, rawset, rawget)
//!
//! Completely blocked:
//! - Coroutines (coroutine)
//! - Debug facilities (debug)
//! - Package management (package)

pub mod docs;
pub mod output;
pub mod packages;
pub mod sandbox;

/// Creates a sandboxed Lua runtime.
///
/// Returns a Lua VM with policy-based sandboxing applied. See module-level documentation
/// for details on what's allowed and blocked.
///
/// # Example
///
/// ```
/// use onetool::runtime;
///
/// # fn example() -> mlua::Result<()> {
/// let lua = runtime::default()?;
///
/// // Safe operations work
/// lua.load("x = math.sqrt(16)").exec()?;
///
/// // Unsafe operations return nil (denied by default policy)
/// let result: mlua::Value = lua.load("return io.open('file.txt')").eval()?;
/// assert!(matches!(result, mlua::Value::Nil));
/// # Ok(())
/// # }
/// ```
pub fn default() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    sandbox::apply(&lua)?;

    Ok(lua)
}
