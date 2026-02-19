//! Utilities for defining and running the tool using [rig-core](https://github.com/0xPlaygrounds/rig)
//!
//! This module provides a `rig::tool::Tool` implementation for the Lua REPL.
//! Requires the `rig` feature to be enabled.
//!
//! # Usage
//!
//! Due to rig-core's requirement that Tools be `Sync`, this module uses a global
//! mutex-protected REPL instance. You must call `set_repl()` before using the tool:
//!
//! ```no_run
//! use onetool::{Repl, rig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let repl = Repl::new()?;
//! rig::set_repl(repl);
//!
//! let lua_tool = rig::LuaRepl::new();
//! // Use lua_tool with rig agents...
//! # Ok(())
//! # }
//! ```

use crate::repl;
use crate::tool_definition;
use once_cell::sync::Lazy;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static REPL: Lazy<Mutex<Option<repl::Repl>>> = Lazy::new(|| Mutex::new(None));

/// Sets the global REPL instance used by LuaRepl tools.
///
/// This must be called before creating any LuaRepl tools. Can only be called once.
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

#[derive(Deserialize)]
pub struct LuaReplArgs {
    pub source_code: String,
}

#[derive(Serialize)]
pub struct LuaReplOutput {
    pub output: String,
    pub result: String,
}

/// A rig-core Tool implementation for the Lua REPL.
///
/// This tool is stateless and Sync-safe. It accesses the global REPL instance
/// set via `set_repl()`.
#[derive(Clone)]
pub struct LuaRepl;

impl LuaRepl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LuaRepl {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for LuaRepl {
    const NAME: &'static str = tool_definition::NAME;

    type Error = std::convert::Infallible;
    type Args = LuaReplArgs;
    type Output = LuaReplOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: tool_definition::NAME.to_string(),
            description: tool_definition::DESCRIPTION.to_string(),
            parameters: tool_definition::json_schema(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = with_repl(|repl| repl.eval(&args.source_code));

        let eval_outcome = match result {
            Some(Ok(outcome)) => outcome,
            Some(Err(err)) => {
                return Ok(LuaReplOutput {
                    output: String::new(),
                    result: format!("error: REPL evaluation failed: {}", err),
                });
            }
            None => {
                return Ok(LuaReplOutput {
                    output: String::new(),
                    result: "error: REPL not initialized. Call onetool::rig::set_repl() first."
                        .to_string(),
                });
            }
        };

        Ok(LuaReplOutput {
            output: eval_outcome.output.join("\n"),
            result: match eval_outcome.result {
                Ok(values) => values.join("\n"),
                Err(err) => format!("error: {}", err),
            },
        })
    }
}
