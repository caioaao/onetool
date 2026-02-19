//! Utilities for defining and running the tool using [aisdk](https://github.com/lazy-hq/aisdk)
//!
//! This module provides an aisdk tool implementation for the Lua REPL using the `#[tool]` macro.
//! Requires the `aisdk` feature to be enabled.
//!
//! # Usage
//!
//! Due to aisdk's requirement for function-based tools, this module uses a global
//! mutex-protected REPL instance. You must call `set_repl()` before using the tool:
//!
//! ```no_run
//! use onetool::{Repl, aisdk};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let repl = Repl::new()?;
//! aisdk::set_repl(repl);
//!
//! // Now you can use lua_repl() as a tool
//! // let result = aisdk::lua_repl("return 1 + 1".to_string()).await?;
//! # Ok(())
//! # }
//! ```

use crate::repl;
use aisdk::macros::tool;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static REPL: Lazy<Mutex<Option<repl::Repl>>> = Lazy::new(|| Mutex::new(None));

/// Sets the global REPL instance used by the lua_repl tool.
///
/// This must be called before using the lua_repl tool. Can only be called once.
///
/// # Panics
///
/// Panics if the mutex is poisoned.
pub fn set_repl(repl: repl::Repl) {
    let mut guard = REPL.lock().expect("REPL mutex poisoned");
    *guard = Some(repl);
}

/// Gets a reference to the global REPL instance for evaluation.
///
/// Returns None if the REPL hasn't been initialized yet.
fn with_repl<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&repl::Repl) -> R,
{
    let guard = REPL.lock().expect("REPL mutex poisoned");
    guard.as_ref().map(f)
}

#[tool]
/// Execute Lua code in a long-lived sandboxed REPL environment.
///
/// **Capabilities:**
/// - Expression evaluation with return values
/// - print() output capture (appears in tool response)
/// - Persistent state between executions (variables, functions, tables)
/// - Safe operations: string, table, math, utf8, os.time, os.date
/// - Documentation: available via global `docs` variable
///
/// **Restrictions:**
/// - No file I/O or network access
/// - No OS command execution
/// - No code loading (require, load, loadfile)
/// - No dangerous metatable operations
///
/// **Environment:**
/// - Sandboxed Lua 5.4
///
/// **Example:**
/// ```lua
/// x = 10
/// y = 20
/// print("Sum:", x + y)
/// return x + y
/// ```
pub fn lua_repl(source_code: String) -> aisdk::core::Tool {
    let result = with_repl(|repl| repl.eval(&source_code));

    let eval_outcome = match result {
        Some(Ok(outcome)) => outcome,
        Some(Err(err)) => {
            return Err(format!("REPL evaluation failed: {}", err));
        }
        None => {
            return Err("REPL not initialized. Call onetool::aisdk::set_repl() first.".to_string());
        }
    };

    // Format response as JSON-like string
    let output = eval_outcome.output.join("");
    let result = match eval_outcome.result {
        Ok(values) => {
            if values.is_empty() {
                String::new()
            } else {
                values.join("\n")
            }
        }
        Err(err) => format!("error: {}", err),
    };

    // Return formatted output
    if output.is_empty() && result.is_empty() {
        Ok("(no output or result)".to_string())
    } else if output.is_empty() {
        Ok(format!("Result: {}", result))
    } else if result.is_empty() {
        Ok(format!("Output: {}", output.trim_end()))
    } else {
        Ok(format!(
            "Output: {}\nResult: {}",
            output.trim_end(),
            result
        ))
    }
}
