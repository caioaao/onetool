//! Example demonstrating onetool integration with MCP (Model Context Protocol).
//!
//! This example creates an MCP server that exposes a Lua REPL tool.
//! MCP clients (like Claude Desktop, Cline, etc.) can connect to this server
//! and use the lua_repl tool to execute sandboxed Lua code.
//!
//! Run with: cargo run --features mcp --example mcp-basic
//!
//! # Usage with Claude Desktop
//!
//! Add this to your Claude Desktop configuration:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "onetool": {
//!       "command": "cargo",
//!       "args": ["run", "--features", "mcp", "--example", "mcp-basic"]
//!     }
//!   }
//! }
//! ```

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for observability
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Create the REPL
    let repl = onetool::Repl::new().map_err(|e| e.to_string())?;

    // Create the MCP server with the REPL
    let server = onetool::mcp::LuaReplServer::new(repl);

    tracing::info!("Starting onetool MCP server...");

    // Start the MCP server on stdin/stdout
    onetool::mcp::serve(server).await?;

    Ok(())
}
