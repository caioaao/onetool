//! Utilities for defining and running the tool using mistral.rs
//!
//! This is a convenience module for users of the `mistralrs` crate.
//! Requires the `mistralrs` feature to be enabled.
//!
//! # Usage
//!
//! Create a `LuaRepl` tool by passing a `Repl` instance:
//!
//! ```no_run
//! use onetool::{Repl, mistralrs::LuaRepl};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let repl = Repl::new()?;
//! let lua_repl = LuaRepl::new(repl);
//!
//! // Use with mistralrs
//! let tool_def = lua_repl.definition();
//! // ... use in model requests
//! # Ok(())
//! # }
//! ```

use crate::repl;
use crate::tool_definition;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// A mistralrs tool implementation for the Lua REPL.
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
}

impl LuaRepl {
    /// Returns mistralrs::Tool definition for the Lua REPL
    pub fn definition(&self) -> mistralrs::Tool {
        // Parse json_schema() into HashMap<String, serde_json::Value>
        let schema = tool_definition::json_schema();
        let parameters: HashMap<String, serde_json::Value> =
            serde_json::from_value(schema).expect("Failed to parse schema");

        mistralrs::Tool {
            tp: mistralrs::ToolType::Function,
            function: mistralrs::Function {
                name: tool_definition::NAME.to_string(),
                description: Some(tool_definition::DESCRIPTION.to_string()),
                parameters: Some(parameters),
            },
        }
    }

    /// Executes the tool call and returns the result string
    ///
    /// This method validates the tool call, extracts parameters, evaluates Lua code,
    /// and returns a formatted result string. The result is suitable for use with
    /// `.add_tool_message(result, call_id)`.
    ///
    /// Returns JSON string with format:
    /// - Success: `{"output": "...", "result": "..."}`
    /// - Error: `{"error": "..."}`
    pub fn call(&self, tool_call: &mistralrs::ToolCallResponse) -> String {
        // 1. Validate tool name
        if tool_call.function.name != tool_definition::NAME {
            return json!({
                "error": format!(
                    "Unknown tool: {}. Expected: {}",
                    tool_call.function.name,
                    tool_definition::NAME
                )
            })
            .to_string();
        }

        // 2. Parse arguments JSON string
        let arguments: serde_json::Value = match serde_json::from_str(&tool_call.function.arguments)
        {
            Ok(args) => args,
            Err(err) => {
                return json!({
                    "error": format!("Failed to parse arguments: {}", err)
                })
                .to_string();
            }
        };

        // 3. Extract source_code parameter
        let source_code = match arguments.get(tool_definition::PARAM_SOURCE_CODE) {
            Some(serde_json::Value::String(s)) => s,
            _ => {
                return json!({
                    "error": format!(
                        "Missing or invalid parameter: {}",
                        tool_definition::PARAM_SOURCE_CODE
                    )
                })
                .to_string();
            }
        };

        // 4. Evaluate Lua code
        let eval_outcome = match self.repl.eval(source_code) {
            Ok(outcome) => outcome,
            Err(err) => {
                return json!({
                    "error": format!("REPL evaluation failed: {}", err)
                })
                .to_string();
            }
        };

        // 5. Format and return success response
        json!({
            "output": eval_outcome.output.join("\n"),
            "result": match eval_outcome.result {
                Ok(values) => values.join("\n"),
                Err(err) => format!("error: {}", err)
            }
        })
        .to_string()
    }
}
