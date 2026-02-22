//! Utilities for defining and running the tool using [aisdk](https://github.com/lazy-hq/aisdk)
//!
//! This module provides an aisdk tool implementation for the Lua REPL.
//! Requires the `aisdk` feature to be enabled.
//!
//! # Usage
//!
//! Create a `LuaRepl` tool by passing a `Repl` instance:
//!
//! ```no_run
//! use onetool::{Repl, aisdk::LuaRepl};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let repl = Repl::new()?;
//! let lua_repl = LuaRepl::new(repl);
//!
//! // Use with aisdk
//! // let result = LanguageModelRequest::builder()
//! //     .with_tool(lua_repl.tool())
//! //     ...
//! # Ok(())
//! # }
//! ```

use crate::repl;
use serde_json::Value;
use std::sync::Arc;

/// An aisdk tool implementation for the Lua REPL.
///
/// The tool maintains a reference to a shared `Repl` instance, preserving Lua state
/// across tool invocations.
#[derive(Clone)]
pub struct LuaRepl {
    repl: Arc<repl::Repl>,
}

impl LuaRepl {
    /// Creates a new LuaRepl tool with the given Repl instance.
    ///
    /// The Repl is wrapped in an Arc, allowing the tool to be cloned while sharing
    /// the same underlying Lua runtime state.
    pub fn new(repl: repl::Repl) -> Self {
        Self {
            repl: Arc::new(repl),
        }
    }

    /// Returns a tool that can be used with aisdk's `.with_tool()`.
    ///
    /// The returned tool captures the Repl instance and executes Lua code
    /// when called by the language model.
    pub fn tool(&self) -> aisdk::core::Tool {
        use crate::tool_definition;
        use aisdk::core::{Tool, tools::ToolExecute};

        let repl = Arc::clone(&self.repl);
        let execute_fn = Box::new(move |args: Value| -> Result<String, String> {
            // Extract source_code from JSON args
            let source_code = match args.get("source_code") {
                Some(Value::String(s)) => s.clone(),
                _ => {
                    return Err("Missing or invalid 'source_code' parameter".to_string());
                }
            };

            let eval_outcome = match repl.eval(&source_code) {
                Ok(outcome) => outcome,
                Err(err) => {
                    return Err(format!("REPL evaluation failed: {}", err));
                }
            };

            // Format response
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
                Ok(format!("Output: {}\nResult: {}", output.trim_end(), result))
            }
        });

        // Convert JSON schema to schemars::Schema
        let schema_json = tool_definition::json_schema();
        let input_schema: schemars::Schema = serde_json::from_value(schema_json)
            .expect("Failed to convert JSON schema to schemars::Schema");

        Tool {
            name: tool_definition::NAME.to_string(),
            description: tool_definition::DESCRIPTION.to_string(),
            input_schema,
            execute: ToolExecute::new(execute_fn),
        }
    }
}
