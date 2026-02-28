//! A sandboxed Lua runtime for LLM tool use.
//!
//! onetool embeds Lua 5.4 and restricts dangerous operations (file I/O, code loading,
//! OS commands, metatable manipulation, coroutines) while preserving safe functionality
//! (string, table, math, utf8, os.time, os.date).
//!
//! # Quick Start
//!
//! ```
//! use onetool::Repl;
//!
//!  fn main() -> Result<(), Box<dyn std::error::Error>> {
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
//! # Security
//!
//! The Lua runtime is sandboxed with policy-based access control. Unsafe functions
//! (like `os.execute`, `io.open`) are wrapped and return `nil` on policy denial. Forbidden
//! functions (like `debug`, `coroutine`) are removed entirely. See [`runtime::sandbox`] for details.
//!
//! # Key Modules
//!
//! - [`Repl`]: Main interface for evaluating Lua code
//! - [`runtime`]: Runtime creation and sandboxing
//! - [`tool_definition`]: Tool schema for LLM integration

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
