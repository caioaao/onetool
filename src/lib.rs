//! A sandboxed Lua REPL for LLM tool use.
//!
//! LLM agents typically need dozens of specialized tools (calculator, date formatter,
//! string manipulator, etc.). Each tool requires a round-trip to the provider and you
//! pay for every token exchanged. **onetool replaces them all with a single sandboxed
//! Lua REPL** — the LLM writes code instead of calling single-purpose tools.
//!
//! # Quick Start
//!
//! ```
//! use onetool::Repl;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let repl = Repl::new()?;
//!
//!     let outcome = repl.eval("return 1 + 1")?;
//!
//!     match outcome.result {
//!         Ok(values) => println!("Result: {:?}", values),
//!         Err(error) => println!("Error: {}", error),
//!     }
//!
//!     for line in outcome.output {
//!         print!("{}", line);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Framework Adapters
//!
//! Ready-to-use adapters (each behind a feature flag of the same name):
//!
//! | Feature | Module | Framework |
//! |---------|--------|-----------|
//! | `genai` | `genai` | [genai](https://github.com/jeremychone/rust-genai) multi-provider client |
//! | `mistralrs` | `mistralrs` | [mistral.rs](https://github.com/EricLBuehler/mistral.rs) local inference |
//! | `rig` | `rig` | [rig-core](https://github.com/0xPlaygrounds/rig) modular framework |
//! | `aisdk` | `aisdk` | [aisdk](https://github.com/lazy-hq/aisdk) Vercel AI SDK port |
//! | `mcp` | `mcp` | [MCP](https://modelcontextprotocol.io/) server via rmcp |
//!
//! # Security
//!
//! The Lua runtime is sandboxed with policy-based access control. Functions are
//! categorized into three tiers: **safe** (no check), **unsafe** (wrapped, denied by
//! default), and **forbidden** (removed entirely). See [`runtime::sandbox`] for the
//! full security model and [`runtime::sandbox::DEFAULT_API_SPEC`] for the complete
//! function list.
//!
//! # Extending the Runtime
//!
//! Register custom Rust functions via [`Repl::with_runtime`] (post-init) or
//! [`Repl::new_with`] (pre-built runtime). See their documentation for examples.
//!
//! # Key Types
//!
//! - [`Repl`] — main interface for evaluating Lua code
//! - [`repl::EvalOutcome`] — result of a single evaluation (return values + captured output)
//! - [`ReplError`] — error type for REPL operations
//! - [`runtime`] — runtime creation, sandboxing, output capture, docs, and package paths
//! - [`tool_definition`] — tool name, description, and JSON schema for LLM integration

// -- Flatten
pub use repl::{Repl, ReplError};

// -- Private modules
mod utils;

// -- Public modules
pub mod repl;
pub mod runtime;
pub mod tool_definition;

#[cfg(feature = "genai")]
pub mod genai;

#[cfg(feature = "mistralrs")]
pub mod mistralrs;

#[cfg(feature = "rig")]
pub mod rig;

#[cfg(feature = "aisdk")]
pub mod aisdk;

#[cfg(feature = "mcp")]
pub mod mcp;
