//! Needle in Haystack Example
//!
//! Demonstrates onetool's ability to provide large context to LLMs through Lua variables.
//! Generates 1,000,000 lines of text with a hidden magic number, then asks the LLM to find it.
//!
//! This showcases a key capability: giving LLMs access to large amounts of data without
//! tool recursion - just inject it as a Lua variable and let the LLM search it.
//!
//! Run with: cargo run --features genai --example needle-in-haystack

use tracing_subscriber::EnvFilter;

const MODEL: &str = "gpt-4o-mini";
const MAGIC_NUMBER: u32 = 31597;
const NUM_LINES: usize = 1_000_000;

// System prompt that instructs the LLM to use the context variable
const SYSTEM_PROMPT: &str = r#"You are an assistant with access to a Lua runtime. You can use Lua to process information and compute answers.

A variable called 'context' is available in the Lua environment containing a large text document (string). When answering questions, use Lua string functions to search through and extract information from the context.

Use the Lua tool to write code that processes the context variable to find the information needed."#;

/// Generates context with a hidden magic number at the midpoint
fn generate_context(num_lines: usize, magic_number: u32) -> String {
    let mut lines = Vec::with_capacity(num_lines);

    for i in 0..num_lines {
        if i == num_lines / 2 {
            // Insert magic number at midpoint
            lines.push(format!("The magic number is {}", magic_number));
        } else {
            lines.push(format!("Line {} contains random filler text data", i));
        }
    }

    lines.join("\n")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber (controlled via RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    println!("\n=== Needle in Haystack Example ===\n");

    // Generate large context
    println!("Generating context with {} lines...", NUM_LINES);
    let context = generate_context(NUM_LINES, MAGIC_NUMBER);
    println!("Context generated: ~{} MB\n", context.len() / (1024 * 1024));

    // Create REPL and inject context as a global Lua variable
    // This is the key pattern: large data accessible without tool recursion
    let repl = onetool::Repl::new()?;
    repl.with_runtime(|lua| {
        lua.globals().set("context", context.as_str())?;
        Ok(())
    })?;

    // Set up genai client and tool
    let genai_client = genai::Client::default();
    let lua_repl = onetool::genai::LuaRepl::new(&repl);

    // Create initial chat request with system prompt and user question
    let question = "I'm looking for a magic number. What is it?";
    let chat_req = genai::chat::ChatRequest::new(vec![
        genai::chat::ChatMessage::system(SYSTEM_PROMPT),
        genai::chat::ChatMessage::user(question),
    ])
    .with_tools(vec![lua_repl.definition()]);

    println!("Asking: \"{}\"\n", question);

    // First LLM call to get tool calls
    let chat_res = genai_client
        .exec_chat(MODEL, chat_req.clone(), None)
        .await?;

    let tool_calls = chat_res.into_tool_calls();

    if tool_calls.is_empty() {
        return Err("Expected tool calls in the response".into());
    }

    // Execute the tool call
    let tool_response = lua_repl.call(&tool_calls[0]);

    // Append tool call and response, then get final answer
    let chat_req = chat_req
        .append_message(tool_calls)
        .append_message(tool_response);

    // Second LLM call to get final answer
    let chat_res = genai_client.exec_chat(MODEL, chat_req, None).await?;

    // Display result
    let answer = chat_res.first_text().unwrap_or("");
    println!("=== LLM Response ===");
    println!("{}\n", answer);

    // Verify
    println!("=== Verification ===");
    println!("Expected magic number: {}", MAGIC_NUMBER);
    if answer.contains(&MAGIC_NUMBER.to_string()) {
        println!("✓ SUCCESS: Answer contains the expected magic number!");
    } else {
        println!("✗ FAILURE: Answer does not contain the expected magic number");
    }

    Ok(())
}
