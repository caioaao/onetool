//! Execution timeout for Lua code.
//!
//! This module provides a mechanism to limit execution time of Lua code using
//! instruction-count-based hooks. When the configured duration elapses, the Lua VM
//! is interrupted via a `RuntimeError`.

use std::time::{Duration, Instant};

/// RAII guard that removes the Lua hook on drop.
struct HookGuard<'a>(&'a mlua::Lua);

impl Drop for HookGuard<'_> {
    fn drop(&mut self) {
        self.0.remove_hook();
    }
}

/// Executes a closure with a timeout.
///
/// Sets up an instruction-count hook that checks elapsed time every 128 instructions.
/// If the duration elapses, the Lua VM is interrupted with a `RuntimeError`.
///
/// # Example
///
/// ```
/// use onetool::runtime::timeout;
/// use std::time::Duration;
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let result = timeout::with_timeout(&lua, Duration::from_millis(100), |lua| {
///     lua.load("while true do end").exec()
/// });
///
/// assert!(result.is_err());
/// # Ok(())
/// # }
/// ```
pub fn with_timeout<F, R>(lua: &mlua::Lua, duration: Duration, f: F) -> mlua::Result<R>
where
    F: FnOnce(&mlua::Lua) -> mlua::Result<R>,
{
    let start = Instant::now();

    let _ = lua.set_hook(
        mlua::HookTriggers::new().every_nth_instruction(128),
        move |_lua, _debug| {
            if start.elapsed() >= duration {
                Err(mlua::Error::RuntimeError("execution timed out".to_string()))
            } else {
                Ok(mlua::VmState::Continue)
            }
        },
    );

    let _guard = HookGuard(lua);
    f(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infinite_loop_times_out() {
        let lua = mlua::Lua::new();
        let result = with_timeout(&lua, Duration::from_millis(100), |lua| {
            lua.load("while true do end").exec()
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_fast_code_completes() {
        let lua = mlua::Lua::new();
        let result = with_timeout(&lua, Duration::from_secs(5), |lua| {
            lua.load("return 1 + 1").eval::<i32>()
        });

        assert_eq!(result.unwrap(), 2);
    }

    #[test]
    fn test_non_timeout_errors_propagate() {
        let lua = mlua::Lua::new();
        let result = with_timeout(&lua, Duration::from_secs(5), |lua| {
            lua.load(r#"error("custom error")"#).exec()
        });

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("custom error"));
    }

    #[test]
    fn test_hook_removed_after_execution() {
        let lua = mlua::Lua::new();

        // Run with a very short timeout
        let _ = with_timeout(&lua, Duration::from_millis(50), |lua| {
            lua.load("return 1").eval::<i32>()
        });

        // After with_timeout returns, the hook should be removed.
        // A long-running loop should complete without timeout.
        let result: Result<(), mlua::Error> = lua
            .load("local x = 0; for i = 1, 1000000 do x = x + 1 end")
            .exec();
        assert!(result.is_ok());
    }

    #[test]
    fn test_callback_error_chain_detected() {
        let lua = mlua::Lua::new();

        // Create a Rust callback that calls Lua code which will timeout
        let inner_fn = lua
            .create_function(|lua, ()| {
                lua.load("while true do end").exec()?;
                Ok(())
            })
            .unwrap();
        lua.globals().set("inner_fn", inner_fn).unwrap();

        let result = with_timeout(&lua, Duration::from_millis(100), |lua| {
            lua.load("inner_fn()").exec()
        });

        assert!(result.is_err());
    }
}
