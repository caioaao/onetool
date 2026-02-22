//! Utilities for defining and running the tool using [MCP (Model Context Protocol)](https://modelcontextprotocol.io/)
//!
//! This module provides an MCP server implementation for the Lua REPL using the rmcp SDK.
//! Requires the `mcp` feature to be enabled.
//!
//! # Usage
//!
//! Create a REPL instance and use it to create an MCP server:
//!
//! ```no_run
//! use onetool::{Repl, mcp};
//!
//! # #[tokio::main]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let repl = Repl::new()?;
//! let server = mcp::LuaReplServer::new(repl);
//!
//! // Start the MCP server on stdin/stdout
//! mcp::serve(server).await?;
//! # Ok(())
//! # }
//! ```

use crate::repl;
use crate::tool_definition;
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use rmcp::{
    Json, ServerHandler, ServiceExt,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{ErrorCode, ErrorData as McpError, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// Input parameters for the Lua REPL tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LuaReplRequest {
    #[schemars(description = "Lua source code to execute in the sandboxed REPL environment")]
    pub source_code: String,
}

/// Output schema for Lua REPL evaluations.
///
/// This structured format provides machine-readable responses for MCP clients.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LuaReplOutput {
    /// Lines of output from print() calls
    pub output: Vec<String>,
    /// Evaluation result: either return values or error message
    pub result: Result<Vec<String>, String>,
}

/// MCP server for the Lua REPL tool.
///
/// This server exposes a single "lua_repl" tool that evaluates Lua code
/// in a sandboxed environment.
#[derive(Clone)]
pub struct LuaReplServer {
    repl: Arc<repl::Repl>,
    tool_router: ToolRouter<LuaReplServer>,
}

#[tool_router]
impl LuaReplServer {
    /// Creates a new LuaReplServer instance with the given REPL.
    pub fn new(repl: repl::Repl) -> Self {
        Self {
            repl: Arc::new(repl),
            tool_router: Self::tool_router(),
        }
    }

    /// Execute Lua code in a long-lived sandboxed REPL environment.
    ///
    /// **Capabilities:**
    /// - Expression evaluation with return values
    /// - print() output capture (appears in tool response)
    /// - Persistent state between executions (variables, functions, tables)
    /// - Safe operations: string, table, math, utf8, os.time, os.date
    /// - Documentation: available via global `docs` variable
    ///
    /// **Restrictions:**
    /// - No file I/O or network access
    /// - No OS command execution
    /// - No code loading (require, load, loadfile)
    /// - No dangerous metatable operations
    ///
    /// **Environment:**
    /// - Sandboxed Lua 5.4
    ///
    /// **Example:**
    /// ```lua
    /// x = 10
    /// y = 20
    /// print("Sum:", x + y)
    /// return x + y
    /// ```
    #[tool(description = "Execute Lua code in a long-lived sandboxed REPL environment")]
    async fn lua_repl(
        &self,
        params: Parameters<LuaReplRequest>,
    ) -> Result<Json<LuaReplOutput>, McpError> {
        let source_code = params.0.source_code;
        let repl = Arc::clone(&self.repl);

        // Use spawn_blocking to avoid blocking the async runtime
        let eval_outcome = tokio::task::spawn_blocking(move || repl.eval(&source_code))
            .await
            .map_err(|e| McpError {
                code: ErrorCode(-32603),
                message: Cow::from(format!("Task join error: {}", e)),
                data: None,
            })?;

        // Handle evaluation result
        match eval_outcome {
            Ok(outcome) => {
                // Return successful output as structured JSON
                Ok(Json(LuaReplOutput {
                    output: outcome.output,
                    result: outcome.result.map_err(|e| e.to_string()),
                }))
            }
            Err(err) => {
                // Return evaluation error as structured output
                Ok(Json(LuaReplOutput {
                    output: Vec::new(),
                    result: Err(format!("REPL evaluation failed: {}", err)),
                }))
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for LuaReplServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(format!(
                "This server provides a sandboxed Lua REPL tool.\n\n{}",
                tool_definition::DESCRIPTION
            )),
            ..Default::default()
        }
    }
}

/// Convenience function to start an MCP server on stdio transport.
///
/// This function runs the given LuaReplServer on stdin/stdout,
/// which is the standard transport for MCP servers.
///
/// # Example
///
/// ```no_run
/// use onetool::{Repl, mcp};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let repl = Repl::new()?;
///     let server = mcp::LuaReplServer::new(repl);
///     mcp::serve(server).await?;
///     Ok(())
/// }
/// ```
pub async fn serve(server: LuaReplServer) -> Result<(), Box<dyn std::error::Error>> {
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
