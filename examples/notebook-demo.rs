//! Interactive Notebook-style Example
//!
//! This is an **interactive showcase** demonstrating the onetool library with beautiful,
//! notebook-style formatting similar to Jupyter notebooks or Org-mode. Ask questions
//! and watch as the LLM generates Lua code, executes it, and explains the results!
//!
//! ## Usage
//!
//! ```bash
//! # Set your API key
//! export OPENAI_API_KEY=your_key_here
//!
//! # Run the interactive notebook
//! cargo run --features notebook_demo --example notebook-demo
//!
//! # Then type your prompts at the >>> prompt:
//! >>> Calculate the first 10 Fibonacci numbers
//! >>> What's the sum of squares from 1 to 100?
//! >>> exit
//! ```
//!
//! ## Architecture
//!
//! This example demonstrates three key onetool concepts:
//! 1. **REPL creation**: `onetool::Repl::new()` for sandboxed Lua runtime
//! 2. **GenAI integration**: `onetool::genai::LuaRepl` adapter for tool handling
//! 3. **Agentic loops**: Iterative tool calling until final text response
//!
//! The notebook formatting is purely cosmetic - focus on the core loop in main().

use std::io::{self, IsTerminal, Write};
use tracing_subscriber::EnvFilter;

const MODEL: &str = "gpt-4o-mini";

// ============================================================================
// Main Demo
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber (controlled via RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Validate API key is set
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable is not set.");
        eprintln!("Please set it before running this example:");
        eprintln!("  export OPENAI_API_KEY=your_key_here");
        return Err("Missing OPENAI_API_KEY".into());
    }

    // Check if we're in a TTY (terminal) for color support
    let use_colors = std::io::stdout().is_terminal();

    print_banner(use_colors);

    // Create the Lua REPL (sandboxed environment)
    let repl = onetool::Repl::new().expect("Failed to create REPL");

    let genai_client = genai::Client::default();

    // Create the tool orchestrator for easier tool handling
    let lua_repl = onetool::genai::LuaRepl::new(&repl);

    // Maintain conversation history for context
    let mut conversation_history: Vec<genai::chat::ChatMessage> = Vec::new();

    // Show usage instructions
    let dim_cyan = if use_colors { "\x1b[2m\x1b[36m" } else { "" };
    let reset = if use_colors { "\x1b[0m" } else { "" };
    println!(
        "{}Type your prompt and press Enter. Type 'exit' or 'quit' to end the session.{}",
        dim_cyan, reset
    );
    println!();

    // Interactive loop
    loop {
        match read_user_input(use_colors)? {
            UserInput::Empty => continue,
            UserInput::Exit => {
                let bold_green = if use_colors { "\x1b[1m\x1b[32m" } else { "" };
                let reset = if use_colors { "\x1b[0m" } else { "" };
                println!("\n{}👋 Goodbye!{}\n", bold_green, reset);
                break;
            }
            UserInput::Command(user_prompt) => {
                process_command(
                    user_prompt,
                    use_colors,
                    &mut conversation_history,
                    &genai_client,
                    &lua_repl,
                )
                .await?;
            }
        }
    }

    Ok(())
}

// Process a user command through the agentic loop
async fn process_command(
    user_prompt: String,
    use_colors: bool,
    conversation_history: &mut Vec<genai::chat::ChatMessage>,
    genai_client: &genai::Client,
    lua_repl: &onetool::genai::LuaRepl<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add user message to conversation history
    conversation_history.push(genai::chat::ChatMessage::user(user_prompt));

    // Agentic loop: keep calling the model until it returns a text response
    loop {
        let chat_req = genai::chat::ChatRequest::new(conversation_history.clone())
            .with_tools(vec![lua_repl.definition()]);

        tracing::debug!("Requesting response from model");
        let chat_res = genai_client.exec_chat(MODEL, chat_req, None).await?;

        // Check if we got tool calls or a text response
        let tool_calls = chat_res.clone().into_tool_calls();

        if tool_calls.is_empty() {
            // No tool calls - this is a final text response
            let answer = chat_res.first_text().unwrap_or("(no response)");
            print_cell(
                answer,
                &CellRenderer::new(CellType::Answer, use_colors),
                "(no answer)",
            );
            conversation_history.push(genai::chat::ChatMessage::assistant(answer));
            break;
        }

        // We have tool calls - execute them
        conversation_history.push(tool_calls.clone().into());

        for tool_call in &tool_calls {
            // Extract and display the generated code
            let source_code = match tool_call.fn_arguments.get("source_code") {
                Some(serde_json::Value::String(code)) => code.as_str(),
                _ => {
                    print_cell(
                        "Tool call missing 'source_code' parameter",
                        &CellRenderer::new(CellType::Error, use_colors),
                        "",
                    );
                    continue;
                }
            };

            print_cell(
                source_code,
                &CellRenderer::new(CellType::Code, use_colors),
                "(no code generated)",
            );

            // Execute the tool call
            tracing::debug!("Executing tool call");
            let tool_response = lua_repl.call(tool_call);

            // Parse and display the execution output
            match serde_json::from_str::<serde_json::Value>(&tool_response.content) {
                Ok(response_json) => {
                    match parse_tool_response(&response_json) {
                        Ok(output) => {
                            print_cell(
                                &output,
                                &CellRenderer::new(CellType::Output, use_colors),
                                "(no output)",
                            );
                        }
                        Err(error) => {
                            print_cell(&error, &CellRenderer::new(CellType::Error, use_colors), "");
                        }
                    }
                    conversation_history.push(tool_response.into());
                }
                Err(e) => {
                    print_cell(
                        &format!("Failed to parse tool response: {}", e),
                        &CellRenderer::new(CellType::Error, use_colors),
                        "",
                    );
                    conversation_history.push(tool_response.into());
                }
            }
        }

        // Loop back to get the next response (might be more tool calls or final answer)
    }

    Ok(())
}

// ============================================================================
// Notebook Cell Formatting
// ============================================================================

const CELL_WIDTH: usize = 100;

/// Pads or truncates a line to fit within the specified width.
/// This ensures consistent box alignment in the notebook output.
fn pad_and_truncate_line(line: &str, width: usize) -> String {
    if line.chars().count() <= width {
        format!("{:width$}", line, width = width)
    } else {
        let truncated: String = line.chars().take(width - 3).collect();
        format!("{}...", truncated)
    }
}

#[derive(Clone, Copy)]
enum CellType {
    Code,
    Output,
    Error,
    Answer,
}

struct CellRenderer {
    icon: &'static str,
    label: &'static str,
    color: &'static str,
    dim_color: &'static str,
    reset: &'static str,
    bold: &'static str,
}

impl CellRenderer {
    fn new(cell_type: CellType, colors_enabled: bool) -> Self {
        let (icon, label, color) = match cell_type {
            CellType::Code => ("💻", "Generated Code", "\x1b[32m"),
            CellType::Output => ("⚡", "Execution Output", "\x1b[33m"),
            CellType::Error => ("❌", "Error", "\x1b[31m"),
            CellType::Answer => ("✨", "Answer", "\x1b[36m"),
        };

        if colors_enabled {
            Self {
                icon,
                label,
                color,
                dim_color: "\x1b[2m",
                reset: "\x1b[0m",
                bold: "\x1b[1m",
            }
        } else {
            Self {
                icon,
                label,
                color: "",
                dim_color: "",
                reset: "",
                bold: "",
            }
        }
    }
}

fn print_cell(content: &str, renderer: &CellRenderer, empty_msg: &str) {
    let width = CELL_WIDTH;
    let content_width = width - 4;

    // Print header
    println!(
        "{}{}{} {}{}",
        renderer.bold, renderer.color, renderer.icon, renderer.label, renderer.reset
    );

    // Print top border
    println!(
        "{}{}┌{}┐{}",
        renderer.dim_color,
        renderer.color,
        "─".repeat(width - 2),
        renderer.reset
    );

    // Print content lines
    let content = if content.is_empty() {
        empty_msg
    } else {
        content
    };

    for line in content.lines() {
        let padded = pad_and_truncate_line(line, content_width);
        println!(
            "{}{}│{}{} {} {}│{}",
            renderer.dim_color,
            renderer.color,
            renderer.reset,
            renderer.color,
            padded,
            renderer.dim_color,
            renderer.reset
        );
    }

    // Print bottom border
    println!(
        "{}{}└{}┘{}",
        renderer.dim_color,
        renderer.color,
        "─".repeat(width - 2),
        renderer.reset
    );
    println!();
}

// ============================================================================
// Helper Functions
// ============================================================================

fn print_banner(colors_enabled: bool) {
    let bold_cyan = if colors_enabled {
        "\x1b[1m\x1b[36m"
    } else {
        ""
    };
    let reset = if colors_enabled { "\x1b[0m" } else { "" };

    println!(
        "{}╔══════════════════════════════════════════════════════════════════════════════════════════════════╗{}",
        bold_cyan, reset
    );
    println!(
        "{}║  ONETOOL NOTEBOOK DEMO  •  Interactive LLM-Powered Lua REPL                                      ║{}",
        bold_cyan, reset
    );
    println!(
        "{}╚══════════════════════════════════════════════════════════════════════════════════════════════════╝{}",
        bold_cyan, reset
    );
    println!();
}

enum UserInput {
    Command(String),
    Exit,
    Empty,
}

fn read_user_input(colors_enabled: bool) -> io::Result<UserInput> {
    let bold_blue = if colors_enabled {
        "\x1b[1m\x1b[34m"
    } else {
        ""
    };
    let reset = if colors_enabled { "\x1b[0m" } else { "" };

    print!("{}>>> {}", bold_blue, reset);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let trimmed = input.trim();

    if trimmed.is_empty() {
        Ok(UserInput::Empty)
    } else if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
        Ok(UserInput::Exit)
    } else {
        Ok(UserInput::Command(trimmed.to_string()))
    }
}

/// Parses the tool response JSON and extracts output and result fields.
/// Returns Ok(formatted_output) or Err(error_message).
fn parse_tool_response(response_json: &serde_json::Value) -> Result<String, String> {
    // Check for errors in tool execution
    if let Some(error) = response_json.get("error") {
        if let Some(error_msg) = error.as_str() {
            return Err(error_msg.to_string());
        }
    }

    let output_text = response_json["output"].as_str().unwrap_or("");
    let result_text = response_json["result"].as_str().unwrap_or("");

    let combined_output = if !output_text.is_empty() && !result_text.is_empty() {
        format!("{}\n\nResult: {}", output_text, result_text)
    } else if !output_text.is_empty() {
        output_text.to_string()
    } else if !result_text.is_empty() {
        result_text.to_string()
    } else {
        "(no output or result)".to_string()
    };

    Ok(combined_output)
}
