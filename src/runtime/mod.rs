//! Runtime creation and sandboxing.
//!
//! This module provides functions to create sandboxed Lua runtimes and utilities
//! for output capture, documentation registration, and package path management.
//!
//! See [`sandbox`] for the full security model and API specification.

pub mod docs;
pub mod output;
pub mod packages;
pub mod sandbox;
pub mod timeout;

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
