//! Utilities for defining and running the tool using [genai](https://github.com/jeremychone/rust-genai)
//!
//! This adapter provides output truncation to prevent large outputs from bloating the LLM's context window.
//! By default, both `output` (from `print()`) and `result` (from `return`) are truncated to 50,000 characters.

/// Creates a genai::chat::Tool instance for the REPL.
///
/// This is a convenience function for users of the `genai` crate.
/// Requires the `genai` feature to be enabled.
use crate::repl;
use crate::tool_definition;
use crate::utils;
use serde_json::json;

pub struct LuaRepl<'a> {
    repl: &'a repl::Repl,
    options: LuaReplOptions,
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

impl<'a> LuaRepl<'a> {
    const DEFAULT_OPTIONS: LuaReplOptions = LuaReplOptions {
        max_output_len: 50_000,
    };

    /// Creates a new LuaRepl with default options (50,000 character truncation limit).
    pub fn new(repl: &'a repl::Repl) -> Self {
        Self::new_with(repl, Self::DEFAULT_OPTIONS)
    }

    /// Creates a new LuaRepl with custom options.
    ///
    /// Use this to configure custom truncation limits or other options.
    pub fn new_with(repl: &'a repl::Repl, options: LuaReplOptions) -> Self {
        Self { repl, options }
    }

    pub fn definition(&self) -> genai::chat::Tool {
        genai::chat::Tool::new(tool_definition::NAME)
            .with_description(tool_definition::DESCRIPTION)
            .with_schema(tool_definition::json_schema())
    }

    /// Calls the tool with the provided tool call and returns a response.
    ///
    /// This method validates the tool call, extracts parameters, evaluates the Lua code,
    /// and returns a properly formatted response. All errors are embedded in the response
    /// content as JSON, following the "graceful degradation" pattern.
    ///
    /// **Note:** Both `output` and `result` are automatically truncated according to the
    /// configured `max_output_len` option to prevent context window overflow.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use onetool::{Repl, genai::LuaRepl};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new()?;
    /// let lua_repl = LuaRepl::new(&repl);
    ///
    /// // Tool call would come from LLM client
    /// // let tool_call = ...;
    /// // let response = lua_repl.call(&tool_call);
    /// # Ok(())
    /// # }
    /// ```
    pub fn call(&self, tool_call: &genai::chat::ToolCall) -> genai::chat::ToolResponse {
        // 1. Validate tool name
        if tool_call.fn_name != tool_definition::NAME {
            return Self::build_error_response(
                tool_call.call_id.clone(),
                format!(
                    "Unknown tool: {}. Expected: {}",
                    tool_call.fn_name,
                    tool_definition::NAME
                ),
            );
        }

        // 2. Extract source_code parameter
        let source_code = match &tool_call.fn_arguments[tool_definition::PARAM_SOURCE_CODE] {
            serde_json::Value::String(s) => s,
            _ => {
                return Self::build_error_response(
                    tool_call.call_id.clone(),
                    format!(
                        "Missing or invalid parameter: {}",
                        tool_definition::PARAM_SOURCE_CODE
                    ),
                );
            }
        };

        // 3. Evaluate Lua code
        let eval_outcome = match self.repl.eval(source_code) {
            Ok(outcome) => outcome,
            Err(err) => {
                return Self::build_error_response(
                    tool_call.call_id.clone(),
                    format!("REPL evaluation failed: {}", err),
                );
            }
        };

        let truncated_output =
            utils::truncate_output(&eval_outcome.output.join("\n"), self.options.max_output_len);

        let full_result = match eval_outcome.result {
            Ok(values) => values.join("\n"),
            Err(err) => format!("error: {}", err),
        };

        let truncated_result = utils::truncate_output(&full_result, self.options.max_output_len);

        // 4. Build success response
        genai::chat::ToolResponse::new(
            tool_call.call_id.clone(),
            json!({
                "output": truncated_output,
                "result": truncated_result
            })
            .to_string(),
        )
    }

    /// Builds an error response with the given error message.
    fn build_error_response(call_id: String, error_message: String) -> genai::chat::ToolResponse {
        genai::chat::ToolResponse::new(call_id, json!({ "error": error_message }).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Repl;

    #[test]
    fn test_output_truncation_with_long_output() {
        // Setup
        let repl = Repl::new().expect("Failed to create REPL");
        let options = LuaReplOptions {
            max_output_len: 100,
        };
        let lua_repl = LuaRepl::new_with(&repl, options);

        // Create a mock tool call with code that generates long output
        let long_output_code = r#"
            print(string.rep("A", 500))
            return "result"
        "#;

        let tool_call = genai::chat::ToolCall {
            call_id: "test-123".to_string(),
            fn_name: "lua_repl".to_string(),
            fn_arguments: serde_json::json!({
                "source_code": long_output_code
            }),
            thought_signatures: None,
        };

        // Execute
        let response = lua_repl.call(&tool_call);

        // Parse response
        let response_json: serde_json::Value =
            serde_json::from_str(&response.content).expect("Invalid JSON");

        let output = response_json["output"].as_str().unwrap();

        // Verify truncation
        assert_eq!(output.chars().count(), 100);
        assert!(output.ends_with("...\n(output truncated)"));
    }
}
