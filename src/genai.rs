//! Utilities for defining and running the tool using [genai](https://github.com/jeremychone/rust-genai)

/// Creates a genai::chat::Tool instance for the REPL.
///
/// This is a convenience function for users of the `genai` crate.
/// Requires the `genai` feature to be enabled.
use crate::tool_definition;

pub fn build_tool() -> genai::chat::Tool {
    genai::chat::Tool::new(tool_definition::NAME)
        .with_description(tool_definition::DESCRIPTION)
        .with_schema(tool_definition::json_schema())
}
