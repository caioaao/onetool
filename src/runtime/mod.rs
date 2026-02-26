//! Runtime creation and sandboxing.
//!
//! This module provides functions to create sandboxed Lua runtimes. The sandbox
//! restricts dangerous operations while preserving safe Lua standard library
//! functionality.
//!
//! # Security Model
//!
//! Sandboxed runtimes block:
//! - File I/O (io, file)
//! - Code loading (require, dofile, load, loadfile, package)
//! - OS commands (os.execute, os.getenv, etc.)
//! - Metatable manipulation (getmetatable, setmetatable, rawset, rawget)
//! - Coroutines
//! - Garbage collection control
//!
//! Sandboxed runtimes allow:
//! - String manipulation (string.*)
//! - Table operations (table.*)
//! - Math functions (math.*)
//! - UTF-8 support (utf8.*)
//! - Safe OS functions (os.time, os.date)
//!
//! Blocked operations fail with "attempt to call a nil value" errors.

pub mod docs;
pub mod onetool_api;
pub mod output;
pub mod packages;
pub mod policy;
pub mod sandbox;
pub mod sandbox_v2;

pub use onetool_api::*;

/// Creates a sandboxed Lua runtime.
///
/// Returns a Lua VM with dangerous operations blocked. See module-level documentation
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
/// // Dangerous operations fail
/// let result = lua.load("io.open('file.txt')").exec();
/// assert!(result.is_err());
/// # Ok(())
/// # }
/// ```
pub fn default() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    sandbox::apply(&lua)?;

    Ok(lua)
}
