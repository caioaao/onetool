//! Needle in Haystack Example
//!
//! Demonstrates onetool's ability to provide large context to LLMs through Lua.
//! Generates massive context (pseudo-random text with a hidden magic number) and tests
//! the LLM's ability to find the hidden information using Lua to search through
//! a global context variable.
//!
//! This showcases a key capability: giving LLMs access to large amounts of
//! structured data without tool recursion, just through simple variable access.
//!
//! Run with: cargo run --features genai --example needle-in-haystack

use std::time::{SystemTime, UNIX_EPOCH};
use tracing_subscriber::EnvFilter;

const MODEL: &str = "gpt-4o-mini";

// System prompt that instructs the LLM to use the context variable
const SYSTEM_PROMPT: &str = r#"You are an assistant with access to a Lua runtime. You can use Lua to process information and compute answers.

A variable called 'context' is available in the Lua environment containing a large text document (string). When answering questions, use Lua string functions to search through and extract information from the context.

Use the Lua tool to write code that processes the context variable to find the information needed."#;

/// Simple pseudo-random number generator using Linear Congruential Generator
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        // LCG parameters from Numerical Recipes
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state
    }

    fn gen_range(&mut self, min: usize, max: usize) -> usize {
        min + (self.next() as usize % (max - min + 1))
    }
}

/// Generates massive context with a hidden magic number
///
/// Creates `num_lines` lines of pseudo-random words from a fixed set. Each line has
/// 3-8 words. Inserts "The magic number is {magic_number}" at a calculated
/// position (around 50% through the document).
///
/// Returns: (generated context string, line position where magic number was inserted)
fn generate_massive_context(num_lines: usize, magic_number: u32) -> (String, usize) {
    // Use current time as seed for pseudo-randomness
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut rng = SimpleRng::new(seed);
    let words = [
        "blah",
        "random",
        "text",
        "data",
        "content",
        "information",
        "sample",
    ];

    let mut lines = Vec::with_capacity(num_lines);

    // Generate pseudo-random lines
    for _ in 0..num_lines {
        let word_count = rng.gen_range(3, 8);
        let line_words: Vec<&str> = (0..word_count)
            .map(|_| {
                let idx = rng.next() as usize % words.len();
                words[idx]
            })
            .collect();
        lines.push(line_words.join(" "));
    }

    // Insert magic number at a position between 40-60% through
    let insert_pos = rng.gen_range(num_lines * 40 / 100, num_lines * 60 / 100);
    lines[insert_pos] = format!("The magic number is {}", magic_number);

    (lines.join("\n"), insert_pos)
}

/// Agent that uses Lua to search through context
struct Agent {
    repl: onetool::Repl,
    genai_client: genai::Client,
}

impl Agent {
    /// Creates a new agent with the given context injected into Lua
    fn new(context: String) -> Result<Self, Box<dyn std::error::Error>> {
        let repl = onetool::Repl::new()?;

        // Inject context as a global Lua variable
        repl.with_runtime(|lua| {
            lua.globals().set("context", context.as_str())?;
            Ok(())
        })?;

        let genai_client = genai::Client::default();

        Ok(Agent { repl, genai_client })
    }

    /// Asks the agent a question and returns the answer
    async fn call(&self, question: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Create LuaRepl on-demand
        let lua_repl = onetool::genai::LuaRepl::new(&self.repl);

        // Create initial chat request with system prompt and user question
        let chat_req = genai::chat::ChatRequest::new(vec![
            genai::chat::ChatMessage::system(SYSTEM_PROMPT),
            genai::chat::ChatMessage::user(question),
        ])
        .with_tools(vec![lua_repl.definition()]);

        // First LLM call to get tool calls
        let chat_res = self
            .genai_client
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
        let chat_res = self.genai_client.exec_chat(MODEL, chat_req, None).await?;

        // Extract text content from response
        let content = chat_res.first_text().unwrap_or("").to_string();

        Ok(content)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber (controlled via RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Generate magic number from current timestamp (more interesting than a constant)
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let magic_number = 1_000_000 + (timestamp % 8_999_999) as u32;

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║          Needle in Haystack Example                      ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // Generate context
    println!("Generating context with 10,000 lines...");
    let (context, insert_pos) = generate_massive_context(10_000, magic_number);
    println!("✓ Context generated (~{}KB)", context.len() / 1024);
    println!("✓ Magic number inserted at line {}\n", insert_pos + 1);

    // Create agent with context
    println!("Creating agent with context...");
    let agent = Agent::new(context)?;
    println!("✓ Agent created\n");

    // Ask the question
    println!("Asking question: \"I'm looking for a magic number. What is it?\"\n");
    let answer = agent
        .call("I'm looking for a magic number. What is it?")
        .await?;

    // Display results
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║                     LLM Response                          ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("{}\n", answer);

    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║                    Verification                           ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("Expected magic number: {}", magic_number);

    // Check if answer contains the expected number
    if answer.contains(&magic_number.to_string()) {
        println!("✓ SUCCESS: Answer contains the expected magic number!");
    } else {
        println!("✗ FAILURE: Answer does not contain the expected magic number");
    }

    Ok(())
}
