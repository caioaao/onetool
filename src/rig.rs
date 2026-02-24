//! Utilities for defining and running the tool using [rig-core](https://github.com/0xPlaygrounds/rig)
//!
//! This module provides a `rig::tool::Tool` implementation for the Lua REPL.
//! Requires the `rig` feature to be enabled.
//!
//! This adapter provides output truncation to prevent large outputs from bloating the LLM's context window.
//! By default, both `output` (from `print()`) and `result` (from `return`) are truncated to 50,000 characters.
//!
//! # Usage
//!
//! Create a `LuaRepl` tool by passing a `Repl` instance. The tool can be cloned and
//! shared across rig agents while maintaining persistent Lua state:
//!
//! ```no_run
//! use onetool::{Repl, rig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let repl = Repl::new()?;
//! let lua_tool = rig::LuaRepl::new(repl);
//! // Use lua_tool with rig agents...
//! # Ok(())
//! # }
//! ```

use crate::repl;
use crate::tool_definition;
use crate::utils;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct LuaReplArgs {
    pub source_code: String,
}

#[derive(Serialize)]
pub struct LuaReplOutput {
    pub output: String,
    pub result: String,
}

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

/// A rig-core Tool implementation for the Lua REPL.
///
/// The tool maintains a reference to a shared `Repl` instance, preserving Lua state
/// across tool invocations. Multiple clones of this tool will share the same Repl.
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
        let eval_outcome = match self.repl.eval(&args.source_code) {
            Ok(outcome) => outcome,
            Err(err) => {
                return Ok(LuaReplOutput {
                    output: String::new(),
                    result: format!("error: REPL evaluation failed: {}", err),
                });
            }
        };

        let truncated_output =
            utils::truncate_output(&eval_outcome.output.join("\n"), self.options.max_output_len);

        let full_result = match eval_outcome.result {
            Ok(values) => values.join("\n"),
            Err(err) => format!("error: {}", err),
        };

        let truncated_result = utils::truncate_output(&full_result, self.options.max_output_len);

        Ok(LuaReplOutput {
            output: truncated_output,
            result: truncated_result,
        })
    }
}
