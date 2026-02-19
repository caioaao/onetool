//! Utilities for defining and running the tool using [genai](https://github.com/jeremychone/rust-genai)

/// Creates a genai::chat::Tool instance for the REPL.
///
/// This is a convenience function for users of the `genai` crate.
/// Requires the `genai` feature to be enabled.
use crate::repl;
use crate::tool_definition;
use serde_json::json;

pub struct Tool<'a> {
    repl: &'a repl::Repl,
}

impl<'a> Tool<'a> {
    pub fn new(repl: &'a repl::Repl) -> Self {
        Self { repl }
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
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use onetool::{Repl, genai::Tool};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new()?;
    /// let tool = Tool::new(&repl);
    ///
    /// // Tool call would come from LLM client
    /// // let tool_call = ...;
    /// // let response = tool.call_tool(&tool_call);
    /// # Ok(())
    /// # }
    /// ```
    pub fn call_tool(&self, tool_call: &genai::chat::ToolCall) -> genai::chat::ToolResponse {
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

        // 4. Build success response
        genai::chat::ToolResponse::new(
            tool_call.call_id.clone(),
            json!({
                "output": eval_outcome.output.join("\n"),
                "result": match eval_outcome.result {
                    Ok(values) => values.join("\n"),
                    Err(err) => format!("error: {}", err)
                }
            })
            .to_string(),
        )
    }

    /// Builds an error response with the given error message.
    fn build_error_response(call_id: String, error_message: String) -> genai::chat::ToolResponse {
        genai::chat::ToolResponse::new(call_id, json!({ "error": error_message }).to_string())
    }
}
