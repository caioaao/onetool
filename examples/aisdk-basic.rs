//! Example demonstrating onetool integration with aisdk.
//!
//! This example creates a Lua REPL tool and uses it with an aisdk agent
//! to solve a simple math problem.
//!
//! Run with: cargo run --features aisdk --example aisdk-basic

use aisdk::core::LanguageModelRequest;
use aisdk::providers::OpenAI;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Initialize the REPL
    let repl = onetool::Repl::new().map_err(|e| e.to_string())?;
    let lua_repl = onetool::aisdk::LuaRepl::new(repl);

    println!("--- Creating aisdk agent with Lua REPL tool");

    let result = LanguageModelRequest::builder()
        .model(OpenAI::gpt_4o())
        .system(
            "You are a helpful assistant that can execute Lua code to solve problems. \
                 Use the lua_repl tool to run calculations and verify your answers.",
        )
        .prompt("What's the sum of the first 10 prime numbers? Use Lua to calculate it.")
        .with_tool(lua_repl.tool())
        .build()
        .generate_text()
        .await?;

    println!("\n--- Agent response:");
    if let Some(text) = result.text() {
        println!("{}", text);
    } else {
        println!("(no text response)");
    }

    Ok(())
}
