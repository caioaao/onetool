//! Custom Functions Extension Example
//!
//! Demonstrates extending onetool's Lua runtime with custom Rust functions.
//! Shows: custom functions, stateful closures, error handling, docs registration.
//!
//! Run with: cargo run --example custom-functions

use onetool::{Repl, runtime};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

// ============================================================================
// Section 1: Helper Functions
// ============================================================================
// Rust functions that will be wrapped for Lua

/// Simulates an HTTP GET request (for demonstration purposes)
fn simulate_http_fetch(url: &str) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("Invalid URL: {}", url));
    }

    // Simulate a response (in real use, you'd use reqwest or similar)
    Ok(format!(
        "{{\"url\": \"{}\", \"status\": 200, \"body\": \"Mock response\"}}",
        url
    ))
}

/// Computes SHA256 hash of input string
fn compute_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Validates email address format
fn validate_email(email: &str) -> bool {
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    email_regex.is_match(email)
}

// ============================================================================
// Section 2: Method 1 - Post-Initialization (with_runtime)
// ============================================================================

fn example_with_runtime() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Method 1: Extension via with_runtime() ===\n");
    println!("This method adds functions to an existing REPL instance.");
    println!("Best for: Simple additions after REPL creation\n");

    let repl = Repl::new()?;

    // 1. Simple function with error handling
    repl.with_runtime(|lua| {
        let http_fetch = lua.create_function(|_, url: String| {
            simulate_http_fetch(&url).map_err(|e| mlua::Error::RuntimeError(e))
        })?;
        lua.globals().set("http_fetch", http_fetch)?;
        Ok(())
    })?;
    println!("✓ Registered http_fetch()");

    // 2. Binary data function (hash computation)
    repl.with_runtime(|lua| {
        let hash = lua.create_function(|_, input: String| Ok(compute_sha256(&input)))?;
        lua.globals().set("hash_sha256", hash)?;
        Ok(())
    })?;
    println!("✓ Registered hash_sha256()");

    // 3. Boolean validation function
    repl.with_runtime(|lua| {
        let validate = lua.create_function(|_, email: String| Ok(validate_email(&email)))?;
        lua.globals().set("validate_email", validate)?;
        Ok(())
    })?;
    println!("✓ Registered validate_email()");

    // 4. Stateful module with closure
    let counter = Arc::new(AtomicI32::new(0));
    repl.with_runtime({
        let counter = Arc::clone(&counter);
        move |lua| {
            let module = lua.create_table()?;

            // Increment function
            let c1 = Arc::clone(&counter);
            let inc =
                lua.create_function(move |_, ()| Ok(c1.fetch_add(1, Ordering::SeqCst) + 1))?;
            module.set("increment", inc)?;

            // Get current value
            let c2 = Arc::clone(&counter);
            let get = lua.create_function(move |_, ()| Ok(c2.load(Ordering::SeqCst)))?;
            module.set("get", get)?;

            // Reset counter
            let c3 = Arc::clone(&counter);
            let reset = lua.create_function(move |_, ()| {
                c3.store(0, Ordering::SeqCst);
                Ok(())
            })?;
            module.set("reset", reset)?;

            lua.globals().set("counter", module)?;
            Ok(())
        }
    })?;
    println!("✓ Registered counter module (increment, get, reset)");

    // 5. Register documentation
    repl.with_runtime(|lua| {
        use onetool::runtime::docs::{LuaDoc, LuaDocTyp, register};

        register(
            lua,
            &LuaDoc {
                name: "http_fetch".to_string(),
                typ: LuaDocTyp::Function,
                description: "Simulates HTTP GET request. Usage: http_fetch(url)".to_string(),
            },
        )?;

        register(
            lua,
            &LuaDoc {
                name: "hash_sha256".to_string(),
                typ: LuaDocTyp::Function,
                description: "Computes SHA256 hash. Usage: hash_sha256(string)".to_string(),
            },
        )?;

        register(
            lua,
            &LuaDoc {
                name: "validate_email".to_string(),
                typ: LuaDocTyp::Function,
                description: "Validates email format. Usage: validate_email(email)".to_string(),
            },
        )?;

        register(
            lua,
            &LuaDoc {
                name: "counter".to_string(),
                typ: LuaDocTyp::Scope,
                description: "Stateful counter. Methods: increment(), get(), reset()".to_string(),
            },
        )?;

        Ok(())
    })?;
    println!("✓ Registered documentation for all functions\n");

    // Demo the functions
    demo_functions(&repl)?;

    Ok(())
}

// ============================================================================
// Section 3: Method 2 - Pre-Sandboxing (new_with)
// ============================================================================

fn example_new_with() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Method 2: Extension via new_with() ===\n");
    println!("This method applies sandboxing FIRST, then registers custom functions.");
    println!("Best for: Complex initialization, framework adapters\n");

    let lua = mlua::Lua::new();

    // CRITICAL: Apply sandboxing FIRST (it clears globals, so custom functions must come after)
    runtime::sandbox::apply(&lua)?;
    println!("✓ Applied sandboxing");

    // Register ALL custom functions AFTER sandboxing
    // (functions registered before would be destroyed by globals.clear())
    let http_fetch = lua.create_function(|_, url: String| {
        simulate_http_fetch(&url).map_err(|e| mlua::Error::RuntimeError(e))
    })?;
    lua.globals().set("http_fetch", http_fetch)?;
    println!("✓ Registered http_fetch()");

    let hash = lua.create_function(|_, input: String| Ok(compute_sha256(&input)))?;
    lua.globals().set("hash_sha256", hash)?;
    println!("✓ Registered hash_sha256()");

    let validate = lua.create_function(|_, email: String| Ok(validate_email(&email)))?;
    lua.globals().set("validate_email", validate)?;
    println!("✓ Registered validate_email()");

    // Stateful counter module
    let counter = Arc::new(AtomicI32::new(0));
    let module = lua.create_table()?;

    let c1 = Arc::clone(&counter);
    let inc = lua.create_function(move |_, ()| Ok(c1.fetch_add(1, Ordering::SeqCst) + 1))?;
    module.set("increment", inc)?;

    let c2 = Arc::clone(&counter);
    let get = lua.create_function(move |_, ()| Ok(c2.load(Ordering::SeqCst)))?;
    module.set("get", get)?;

    let c3 = Arc::clone(&counter);
    let reset = lua.create_function(move |_, ()| {
        c3.store(0, Ordering::SeqCst);
        Ok(())
    })?;
    module.set("reset", reset)?;

    lua.globals().set("counter", module)?;
    println!("✓ Registered counter module");

    // Register documentation
    {
        use onetool::runtime::docs::{LuaDoc, LuaDocTyp, register};

        register(
            &lua,
            &LuaDoc {
                name: "http_fetch".to_string(),
                typ: LuaDocTyp::Function,
                description: "Simulates HTTP GET request. Usage: http_fetch(url)".to_string(),
            },
        )?;

        register(
            &lua,
            &LuaDoc {
                name: "hash_sha256".to_string(),
                typ: LuaDocTyp::Function,
                description: "Computes SHA256 hash. Usage: hash_sha256(string)".to_string(),
            },
        )?;

        register(
            &lua,
            &LuaDoc {
                name: "validate_email".to_string(),
                typ: LuaDocTyp::Function,
                description: "Validates email format. Usage: validate_email(email)".to_string(),
            },
        )?;

        register(
            &lua,
            &LuaDoc {
                name: "counter".to_string(),
                typ: LuaDocTyp::Scope,
                description: "Stateful counter. Methods: increment(), get(), reset()".to_string(),
            },
        )?;
    }
    println!("✓ Registered documentation\n");

    let repl = Repl::new_with(lua)?;

    demo_functions(&repl)?;

    Ok(())
}

// ============================================================================
// Section 4: Demo/Testing Functions
// ============================================================================

fn demo_functions(repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing custom functions:\n");

    // Test http_fetch
    println!("1. Testing http_fetch:");
    let result = repl.eval(r#"return http_fetch("https://example.com")"#)?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   Result: {}", vals[0]);
        }
    }

    // Test hash_sha256
    println!("\n2. Testing hash_sha256:");
    let result = repl.eval(r#"return hash_sha256("hello")"#)?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   SHA256('hello') = {}", vals[0]);
        }
    }

    // Test validate_email
    println!("\n3. Testing validate_email:");
    let result = repl.eval(r#"return validate_email("test@example.com")"#)?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   validate_email('test@example.com') = {}", vals[0]);
        }
    }
    let result = repl.eval(r#"return validate_email("invalid-email")"#)?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   validate_email('invalid-email') = {}", vals[0]);
        }
    }

    // Test counter
    println!("\n4. Testing counter module:");
    repl.eval("counter.increment()")?;
    repl.eval("counter.increment()")?;
    repl.eval("counter.increment()")?;
    let result = repl.eval("return counter.get()")?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   After 3 increments: {}", vals[0]);
        }
    }
    repl.eval("counter.reset()")?;
    let result = repl.eval("return counter.get()")?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   After reset: {}", vals[0]);
        }
    }

    // Test documentation
    println!("\n5. Testing documentation system:");
    let result = repl.eval(r#"return docs["http_fetch"]"#)?;
    if let Ok(vals) = &result.result {
        if !vals.is_empty() {
            println!("   docs['http_fetch'] = {}", vals[0]);
        }
    }

    // Test error handling
    println!("\n6. Testing error handling:");
    let result = repl.eval(r#"return http_fetch("invalid-url")"#)?;
    match &result.result {
        Err(err) => println!("   Caught error: {}", err),
        Ok(_) => println!("   Unexpected success"),
    }

    Ok(())
}

// ============================================================================
// Section 5: Interactive Mode
// ============================================================================

fn interactive_mode(repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
    use rustyline::DefaultEditor;

    let mut editor = DefaultEditor::new()?;

    println!("\n=== Interactive Mode ===");
    println!("Try these functions:");
    println!("  - http_fetch('https://example.com')");
    println!("  - hash_sha256('hello world')");
    println!("  - validate_email('test@example.com')");
    println!("  - counter.increment()");
    println!("  - counter.get()");
    println!("  - counter.reset()");
    println!("  - docs['http_fetch']");
    println!("\nPress Ctrl+C or Ctrl+D to exit\n");

    loop {
        match editor.readline("> ") {
            Ok(line) => {
                editor.add_history_entry(&line).unwrap();
                match repl.eval(&line) {
                    Ok(result) => {
                        if !result.output.is_empty() {
                            println!("-- OUTPUT:");
                            for output in &result.output {
                                println!("{}", output);
                            }
                        }
                        match result.result {
                            Ok(vals) if !vals.is_empty() => {
                                println!("-- RESULT:");
                                for val in vals {
                                    println!("{}", val);
                                }
                            }
                            Err(err) => println!("-- ERROR:\n{}", err),
                            _ => {}
                        }
                    }
                    Err(err) => eprintln!("error: {}", err),
                }
            }
            Err(_) => return Ok(()),
        }
    }
}

// ============================================================================
// Section 6: Main
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║      Custom Functions Extension Example for onetool          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    // Show both extension methods
    example_with_runtime()?;
    example_new_with()?;

    // Set up REPL for interactive mode
    println!("\n=== Setting up Interactive REPL ===\n");
    let repl = Repl::new()?;

    // Use Method 1 for setup (simpler)
    let counter = Arc::new(AtomicI32::new(0));

    repl.with_runtime(|lua| {
        // Register http_fetch
        let http_fetch = lua.create_function(|_, url: String| {
            simulate_http_fetch(&url).map_err(|e| mlua::Error::RuntimeError(e))
        })?;
        lua.globals().set("http_fetch", http_fetch)?;

        // Register hash_sha256
        let hash = lua.create_function(|_, input: String| Ok(compute_sha256(&input)))?;
        lua.globals().set("hash_sha256", hash)?;

        // Register validate_email
        let validate = lua.create_function(|_, email: String| Ok(validate_email(&email)))?;
        lua.globals().set("validate_email", validate)?;

        Ok(())
    })?;

    // Register counter module
    repl.with_runtime({
        let counter = Arc::clone(&counter);
        move |lua| {
            let module = lua.create_table()?;

            let c1 = Arc::clone(&counter);
            let inc =
                lua.create_function(move |_, ()| Ok(c1.fetch_add(1, Ordering::SeqCst) + 1))?;
            module.set("increment", inc)?;

            let c2 = Arc::clone(&counter);
            let get = lua.create_function(move |_, ()| Ok(c2.load(Ordering::SeqCst)))?;
            module.set("get", get)?;

            let c3 = Arc::clone(&counter);
            let reset = lua.create_function(move |_, ()| {
                c3.store(0, Ordering::SeqCst);
                Ok(())
            })?;
            module.set("reset", reset)?;

            lua.globals().set("counter", module)?;
            Ok(())
        }
    })?;

    // Register documentation
    repl.with_runtime(|lua| {
        use onetool::runtime::docs::{LuaDoc, LuaDocTyp, register};

        register(
            lua,
            &LuaDoc {
                name: "http_fetch".to_string(),
                typ: LuaDocTyp::Function,
                description: "Simulates HTTP GET request. Usage: http_fetch(url)".to_string(),
            },
        )?;

        register(
            lua,
            &LuaDoc {
                name: "hash_sha256".to_string(),
                typ: LuaDocTyp::Function,
                description: "Computes SHA256 hash. Usage: hash_sha256(string)".to_string(),
            },
        )?;

        register(
            lua,
            &LuaDoc {
                name: "validate_email".to_string(),
                typ: LuaDocTyp::Function,
                description: "Validates email format. Usage: validate_email(email)".to_string(),
            },
        )?;

        register(
            lua,
            &LuaDoc {
                name: "counter".to_string(),
                typ: LuaDocTyp::Scope,
                description: "Stateful counter. Methods: increment(), get(), reset()".to_string(),
            },
        )?;

        Ok(())
    })?;

    // Interactive testing
    interactive_mode(&repl)?;

    Ok(())
}
