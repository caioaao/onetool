//! Example demonstrating onetool integration with rig-core.
//!
//! This example creates a Lua REPL tool and uses it with a rig agent
//! to solve a simple math problem.
//!
//! Run with: cargo run --features rig --example rig-basic

use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::openai;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let repl = onetool::Repl::new().map_err(|e| e.to_string())?;
    onetool::rig::set_repl(repl);

    let lua_tool = onetool::rig::LuaRepl::new();

    let client = openai::Client::from_env();
    let agent = client
        .agent(openai::GPT_4O)
        .preamble(
            "You are a helpful assistant that can execute Lua code to solve problems. \
             Use the lua_repl tool to run calculations and verify your answers.",
        )
        .max_tokens(1024)
        .tool(lua_tool)
        .build();

    println!("--- Asking agent to calculate sum of first 10 prime numbers");

    let response = agent
        .prompt("What's the sum of the first 10 prime numbers? Use Lua to calculate it.")
        .await?;

    println!("\n--- Agent response:");
    println!("{}", response);

    Ok(())
}
