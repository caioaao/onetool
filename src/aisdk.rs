//! Utilities for defining and running the tool using [aisdk](https://github.com/lazy-hq/aisdk)
//!
//! This module provides an aisdk tool implementation for the Lua REPL.
//! Requires the `aisdk` feature to be enabled.
//!
//! This adapter provides output truncation to prevent large outputs from bloating the LLM's context window.
//! By default, both `output` (from `print()`) and `result` (from `return`) are truncated to 50,000 characters.
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
use crate::utils;
use serde_json::Value;
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

/// An aisdk tool implementation for the Lua REPL.
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

    /// Returns a tool that can be used with aisdk's `.with_tool()`.
    ///
    /// The returned tool captures the Repl instance and executes Lua code
    /// when called by the language model.
    ///
    /// **Note:** Both `output` and `result` are automatically truncated according to the
    /// configured `max_output_len` option to prevent context window overflow.
    pub fn tool(&self) -> aisdk::core::Tool {
        use crate::tool_definition;
        use aisdk::core::{Tool, tools::ToolExecute};

        let repl = Arc::clone(&self.repl);
        let options = self.options;
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

            // Format response and apply truncation
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

            let truncated_output = utils::truncate_output(&output, options.max_output_len);
            let truncated_result = utils::truncate_output(&result, options.max_output_len);

            // Return formatted output
            if truncated_output.is_empty() && truncated_result.is_empty() {
                Ok("(no output or result)".to_string())
            } else if truncated_output.is_empty() {
                Ok(format!("Result: {}", truncated_result))
            } else if truncated_result.is_empty() {
                Ok(format!("Output: {}", truncated_output.trim_end()))
            } else {
                Ok(format!(
                    "Output: {}\nResult: {}",
                    truncated_output.trim_end(),
                    truncated_result
                ))
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
