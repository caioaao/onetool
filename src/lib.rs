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
//! The sandboxed runtime blocks file I/O, network access, code loading, and OS command
//! execution. Dangerous Lua globals are set to `nil`, causing attempts to use them to fail
//! with "attempt to call a nil value" errors. See [`runtime::sandbox`] for details.
//!
//! # Key Modules
//!
//! - [`Repl`]: Main interface for evaluating Lua code
//! - [`runtime`]: Runtime creation and sandboxing
//! - [`tool_definition`]: Tool schema for LLM integration

// -- Flatten
pub use repl::Repl;

// -- Public modules
pub mod repl;
pub mod runtime;
pub mod tool_definition;

#[cfg(feature = "genai")]
pub mod genai;
