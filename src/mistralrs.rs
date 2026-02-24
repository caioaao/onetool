//! Utilities for defining and running the tool using mistral.rs
//!
//! This is a convenience module for users of the `mistralrs` crate.
//! Requires the `mistralrs` feature to be enabled.
//!
//! This adapter provides output truncation to prevent large outputs from bloating the LLM's context window.
//! By default, both `output` (from `print()`) and `result` (from `return`) are truncated to 50,000 characters.
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
use crate::utils;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration options for the Lua REPL tool.
///
/// Controls output truncation to prevent extremely large outputs from overwhelming
/// the LLM's context window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuaReplOptions {
    /// Maximum length of output and result strings before truncation (default: 50,000)
    ///
    /// When truncated, the string will end with "...\n(output truncated)"
    pub max_output_len: usize,
}

/// A mistralrs tool implementation for the Lua REPL.
///
/// The tool maintains a reference to a shared `Repl` instance, preserving Lua state
/// across tool invocations.
#[derive(Clone)]
pub struct LuaRepl {
    repl: Arc<repl::Repl>,
    options: LuaReplOptions,
}

impl LuaRepl {
    const DEFAULT_OPTIONS: LuaReplOptions = LuaReplOptions {
        max_output_len: 50_000,
    };

    /// Creates a new LuaRepl tool with default options (50,000 character truncation limit).
    ///
    /// The Repl is wrapped in an Arc, allowing the tool to be cloned while sharing
    /// the same underlying Lua runtime state.
    pub fn new(repl: repl::Repl) -> Self {
        Self::new_with(repl, Self::DEFAULT_OPTIONS)
    }

    /// Creates a new LuaRepl tool with custom options.
    ///
    /// Use this to configure custom truncation limits or other options.
    pub fn new_with(repl: repl::Repl, options: LuaReplOptions) -> Self {
        Self {
            repl: Arc::new(repl),
            options,
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
    /// **Note:** Both `output` and `result` are automatically truncated according to the
    /// configured `max_output_len` option to prevent context window overflow.
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

        // 5. Apply truncation and format success response
        let truncated_output =
            utils::truncate_output(&eval_outcome.output.join("\n"), self.options.max_output_len);

        let full_result = match eval_outcome.result {
            Ok(values) => values.join("\n"),
            Err(err) => format!("error: {}", err),
        };

        let truncated_result = utils::truncate_output(&full_result, self.options.max_output_len);

        json!({
            "output": truncated_output,
            "result": truncated_result
        })
        .to_string()
    }
}
