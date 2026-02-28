//! Lua runtime sandboxing with policy-based access control.
//!
//! This module implements security restrictions for Lua runtime environments.
//! Functions are categorized into three tiers:
//!
//! - **Safe**: No policy check, copied directly (e.g., `os.time`, `string.*`, `math.*`)
//! - **Unsafe**: Wrapped with policy-based access control (e.g., `os.execute`, `io.open`)
//! - **Forbidden**: Removed entirely (e.g., `debug`, `coroutine`, `package`)
//!
//! # Quick Start
//!
//! Use [`apply`] for default sandboxing with `DenyAllPolicy`:
//!
//! ```
//! use onetool::runtime::sandbox;
//!
//! # fn example() -> mlua::Result<()> {
//! let lua = mlua::Lua::new();
//! sandbox::apply(&lua)?;  // Uses DenyAllPolicy
//!
//! // Safe functions work
//! let time: i64 = lua.load("return os.time()").eval()?;
//!
//! // Unsafe functions return nil on denial
//! let result: mlua::Value = lua.load("return os.execute('echo test')").eval()?;
//! assert!(matches!(result, mlua::Value::Nil));
//! # Ok(())
//! # }
//! ```
//!
//! # Custom Policies
//!
//! For custom access control, use [`apply_with_policy`]:
//!
//! ```
//! use std::sync::Arc;
//! use onetool::runtime::sandbox;
//!
//! # fn example() -> mlua::Result<()> {
//! let lua = mlua::Lua::new();
//! let policy = Arc::new(sandbox::policy::DenyAllPolicy{});
//! sandbox::apply_with_policy(&lua, policy, None)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Custom API Specifications
//!
//! Advanced users can define custom API specifications using [`ApiEntry`]:
//!
//! ```
//! use onetool::runtime::sandbox::{ApiEntry, ApiSpec};
//!
//! const CUSTOM_SPEC: ApiSpec = &[
//!     ApiEntry::safe_module("string"),
//!     ApiEntry::Module {
//!         name: "os",
//!         entries: &[ApiEntry::safe_function("time")],
//!     },
//! ];
//! ```
//!
//! # API Surface
//!
//! The complete list of allowed, wrapped, and forbidden functions is defined in
//! [`DEFAULT_API_SPEC`]. Functions not listed in the spec are implicitly forbidden
//! (removed from the environment).

pub mod policy;

use crate::runtime::docs::{self, LuaDoc, LuaDocTyp};
use std::sync::Arc;

// ============================================================================
// API Specification (Compile-time Constants)
// ============================================================================

/// Safety level for a Lua function
/// Note: Functions NOT in the spec are implicitly Forbidden (removed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    /// Function is safe - no policy check, copied directly
    Safe,
    /// Function requires policy check - wrapped with access control
    Unsafe,
}

/// Entry in the API specification (recursive definition with embedded names)
///
/// This structure allows defining the spec as a compile-time constant:
/// - No HashMap initialization overhead
/// - More readable structure
/// - Supports functions and nested modules
#[derive(Debug, Clone)]
pub enum ApiEntry {
    /// A function with a safety level
    Function {
        name: &'static str,
        safety: SafetyLevel,
    },
    /// A module containing more entries (allows nesting)
    Module {
        name: &'static str,
        entries: &'static [ApiEntry],
    },
    /// A safe module - copy entire module without inspection
    SafeModule { name: &'static str },
}

impl ApiEntry {
    pub const fn unsafe_function(name: &'static str) -> Self {
        ApiEntry::Function {
            name,
            safety: SafetyLevel::Unsafe,
        }
    }

    pub const fn safe_function(name: &'static str) -> Self {
        ApiEntry::Function {
            name,
            safety: SafetyLevel::Safe,
        }
    }

    pub const fn safe_module(name: &'static str) -> Self {
        ApiEntry::SafeModule { name }
    }
}

/// Complete API specification - array of entries
pub type ApiSpec = &'static [ApiEntry];

/// Default API specification
pub const DEFAULT_API_SPEC: ApiSpec = &[
    // os module: only time and date are safe
    ApiEntry::Module {
        name: "os",
        entries: &[
            ApiEntry::safe_function("time"),
            ApiEntry::safe_function("date"),
            ApiEntry::unsafe_function("execute"),
            ApiEntry::unsafe_function("remove"),
            ApiEntry::unsafe_function("rename"),
            ApiEntry::unsafe_function("exit"),
            ApiEntry::unsafe_function("getenv"),
            ApiEntry::unsafe_function("setlocale"),
            ApiEntry::unsafe_function("tmpname"),
        ],
    },
    // io module: all functions unsafe
    ApiEntry::Module {
        name: "io",
        entries: &[
            ApiEntry::unsafe_function("open"),
            ApiEntry::unsafe_function("close"),
            ApiEntry::unsafe_function("read"),
            ApiEntry::unsafe_function("write"),
            ApiEntry::unsafe_function("flush"),
            ApiEntry::unsafe_function("lines"),
            ApiEntry::unsafe_function("input"),
            ApiEntry::unsafe_function("output"),
            ApiEntry::unsafe_function("popen"),
            ApiEntry::unsafe_function("tmpfile"),
            ApiEntry::unsafe_function("type"),
        ],
    },
    // Safe modules (copy entire table)
    ApiEntry::safe_module("string"),
    ApiEntry::safe_module("table"),
    ApiEntry::safe_module("math"),
    ApiEntry::safe_module("utf8"),
    // Unsafe global functions (top-level)
    ApiEntry::unsafe_function("load"),
    ApiEntry::unsafe_function("loadstring"),
    ApiEntry::unsafe_function("loadfile"),
    ApiEntry::unsafe_function("dofile"),
    ApiEntry::unsafe_function("require"),
    ApiEntry::unsafe_function("getmetatable"),
    ApiEntry::unsafe_function("setmetatable"),
    ApiEntry::unsafe_function("rawget"),
    ApiEntry::unsafe_function("rawset"),
    ApiEntry::unsafe_function("rawequal"),
    ApiEntry::unsafe_function("rawlen"),
    ApiEntry::unsafe_function("collectgarbage"),
    // Safe global functions
    ApiEntry::safe_function("type"),
    ApiEntry::safe_function("tonumber"),
    ApiEntry::safe_function("tostring"),
    ApiEntry::safe_function("print"),
    ApiEntry::safe_function("ipairs"),
    ApiEntry::safe_function("pairs"),
    ApiEntry::safe_function("next"),
    ApiEntry::safe_function("select"),
    ApiEntry::safe_function("assert"),
    ApiEntry::safe_function("error"),
    ApiEntry::safe_function("pcall"),
    ApiEntry::safe_function("xpcall"),
];

/// Applies sandboxing to an existing Lua runtime using policy-based access control.
///
/// This convenience wrapper uses `DenyAllPolicy` by default, which blocks all unsafe
/// function calls. Functions are categorized as:
/// - **Safe**: No policy check, copied directly (e.g., os.time, os.date, string.*, math.*)
/// - **Unsafe**: Wrapped with policy check, returns nil on denial (e.g., os.execute, io.open)
/// - **Forbidden**: Removed entirely (debug, coroutine, package)
///
/// For custom policies, use [`apply_with_policy`] directly.
///
/// # Example
///
/// ```
/// use onetool::runtime::sandbox;
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// lua.globals().set("custom_value", 42)?;
/// sandbox::apply(&lua)?;
///
/// // Custom globals registered before sandboxing are cleared!
/// // Register custom functions AFTER sandboxing:
/// lua.globals().set("my_function", lua.create_function(|_, ()| Ok(42))?)?;
/// # Ok(())
/// # }
/// ```
pub fn apply(lua: &mlua::Lua) -> mlua::Result<()> {
    let policy = Arc::new(policy::DenyAllPolicy);
    apply_with_policy(lua, policy, None)?;
    register_docs(lua)?;
    Ok(())
}

/// Applies sandboxing with a custom policy and optional API specification.
///
/// This is the lower-level API for users who need custom access control policies.
/// For default sandboxing, use [`apply`] instead.
///
/// # Arguments
/// * `lua` - The Lua environment
/// * `policy` - Custom policy for access control decisions
/// * `api_spec` - Optional custom API spec (uses DEFAULT_API_SPEC if None)
///
/// # Sandboxing Strategy
///
/// Only functions explicitly listed in the API spec are available:
/// - **Safe** functions: Copied directly without policy checks
/// - **Unsafe** functions: Wrapped with policy checks
/// - Functions NOT in spec: Removed entirely (implicit Forbidden)
///
/// **Completely blocked (set to nil):**
/// - `debug` - too dangerous even with policy
/// - `coroutine` - blocked entirely
/// - `package` - blocked entirely
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use onetool::runtime::sandbox;
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let policy = Arc::new(sandbox::policy::DenyAllPolicy{});
/// sandbox::apply_with_policy(&lua, policy, None)?;
///
/// // os.execute is wrapped - returns nil on denial
/// let result: mlua::Value = lua.load("return os.execute('echo test')").eval()?;
///
/// // os.time is safe - works without policy checks
/// let timestamp: i64 = lua.load("return os.time()").eval()?;
/// assert!(timestamp > 0);
/// # Ok(())
/// # }
/// ```
pub fn apply_with_policy<P: policy::Policy + 'static>(
    lua: &mlua::Lua,
    policy: Arc<P>,
    api_spec: Option<ApiSpec>,
) -> mlua::Result<()> {
    let spec = api_spec.unwrap_or(DEFAULT_API_SPEC);
    let globals = lua.globals();

    // Collect which modules are in the spec
    let mut modules_in_spec = std::collections::HashSet::new();
    for entry in spec {
        if let ApiEntry::Module { name, .. } = entry {
            modules_in_spec.insert(*name);
        }
    }
    // Process ALL entries (modules AND functions) through process_entries
    // This eliminates duplication - single code path for all processing
    let processed = process_entries(
        lua, spec, policy, "", // Empty prefix for top-level entries
        &globals,
    )?;

    globals.clear()?;

    processed.for_each(|k: mlua::Value, v: mlua::Value| globals.set(k, v))?;

    Ok(())
}

fn register_docs(lua: &mlua::Lua) -> mlua::Result<()> {
    docs::register(
        lua,
        &LuaDoc {
            name: "os".to_string(),
            typ: LuaDocTyp::Scope,
            description: "Operating system functions (sandboxed)".to_string(),
        },
    )?;
    docs::register(
        lua,
        &LuaDoc {
            name: "os.time".to_string(),
            typ: LuaDocTyp::Function,
            description: "Returns current Unix timestamp".to_string(),
        },
    )?;
    docs::register(
        lua,
        &LuaDoc {
            name: "os.date".to_string(),
            typ: LuaDocTyp::Function,
            description: "Formats date/time. Usage: os.date(format, time?)".to_string(),
        },
    )?;
    // Standard library scopes (no individual functions)
    docs::register(
        lua,
        &LuaDoc {
            name: "string".to_string(),
            typ: LuaDocTyp::Scope,
            description: "String manipulation functions".to_string(),
        },
    )?;
    docs::register(
        lua,
        &LuaDoc {
            name: "table".to_string(),
            typ: LuaDocTyp::Scope,
            description: "Table manipulation functions".to_string(),
        },
    )?;
    docs::register(
        lua,
        &LuaDoc {
            name: "math".to_string(),
            typ: LuaDocTyp::Scope,
            description: "Mathematical functions".to_string(),
        },
    )?;
    docs::register(
        lua,
        &LuaDoc {
            name: "utf8".to_string(),
            typ: LuaDocTyp::Scope,
            description: "UTF-8 support functions".to_string(),
        },
    )?;

    Ok(())
}

/// Process API entries from a source table and return a new processed table
///
/// Generic function that handles the core logic of:
/// - Iterating through ApiEntry items
/// - Handling Safe functions (copy directly)
/// - Handling Unsafe functions (wrap with policy)
/// - Recursively processing nested modules
///
/// Returns a new table with only the entries specified in the API spec.
fn process_entries<P: policy::Policy + 'static>(
    lua: &mlua::Lua,
    entries: &'static [ApiEntry],
    policy: Arc<P>,
    name_prefix: &str,
    source_table: &mlua::Table,
) -> mlua::Result<mlua::Table> {
    let target_table = lua.create_table()?;

    for entry in entries {
        match entry {
            ApiEntry::Function { name, safety } => {
                if let Ok(value) = source_table.get::<mlua::Value>(*name) {
                    match safety {
                        SafetyLevel::Safe => {
                            target_table.set(*name, value)?;
                        }
                        SafetyLevel::Unsafe => {
                            if let mlua::Value::Function(func) = value {
                                let qualified_name = if name_prefix.is_empty() {
                                    name.to_string()
                                } else {
                                    format!("{}.{}", name_prefix, name)
                                };
                                let wrapped = wrap_unsafe_function(
                                    lua,
                                    &qualified_name,
                                    func,
                                    Arc::clone(&policy),
                                )?;
                                target_table.set(*name, wrapped)?;
                            } else {
                                // Not a function, copy directly (constants, etc.)
                                target_table.set(*name, value)?;
                            }
                        }
                    }
                }
            }
            ApiEntry::Module { name, entries } => {
                if let Ok(original_module) = source_table.get::<mlua::Table>(*name) {
                    let qualified_name = if name_prefix.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}.{}", name_prefix, name)
                    };

                    // Recursive call - returns new processed module
                    let processed_module = process_entries(
                        lua,
                        entries,
                        Arc::clone(&policy),
                        &qualified_name,
                        &original_module,
                    )?;

                    target_table.set(*name, processed_module)?;
                }
            }
            ApiEntry::SafeModule { name } => {
                // Copy entire module without inspection
                if let Ok(original_module) = source_table.get::<mlua::Table>(*name) {
                    target_table.set(*name, original_module)?;
                }
            }
        }
    }

    Ok(target_table)
}

/// Wraps an unsafe function with policy-based access control.
///
/// This function creates a wrapper that checks the policy before calling the original function.
/// If the policy denies access, nil is returned. If the policy allows access, the original
/// function is called and its return values are forwarded.
///
/// # Arguments
/// * `lua` - The Lua environment
/// * `function_name` - The qualified name of the function (e.g., "os.execute")
/// * `original_fn` - The original Lua function to wrap
/// * `policy` - The policy to check for access control
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use onetool::runtime::sandbox::{self, policy};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let policy = Arc::new(policy::DenyAllPolicy);
///
/// // Get a Lua function
/// lua.load("function dangerous() return 'unsafe' end").exec()?;
/// let func: mlua::Function = lua.globals().get("dangerous")?;
///
/// // Wrap it with policy check
/// let wrapped = sandbox::wrap_unsafe_function(&lua, "dangerous", func, policy)?;
/// lua.globals().set("safe_dangerous", wrapped)?;
///
/// // Calling it returns nil due to DenyAllPolicy
/// let result: mlua::Value = lua.load("return safe_dangerous()").eval()?;
/// assert!(matches!(result, mlua::Value::Nil));
/// # Ok(())
/// # }
/// ```
pub fn wrap_unsafe_function<P: policy::Policy + 'static>(
    lua: &mlua::Lua,
    function_name: &str,
    original_fn: mlua::Function,
    policy: Arc<P>,
) -> mlua::Result<mlua::Function> {
    let function_name_owned = function_name.to_string();
    lua.create_function(move |_lua, args: mlua::MultiValue| {
        // Check policy with CallFunction action
        let action = policy::Action::CallFunction {
            name: function_name_owned.clone(),
            args: args.clone(),
        };

        let decision = policy.check_access(&action);

        // If denied: print reason to stderr, return nil as MultiValue
        if let policy::Decision::Deny(reason) = decision {
            eprintln!("Access denied: {}", reason);
            return Ok(mlua::MultiValue::from_vec(vec![mlua::Value::Nil]));
        }

        // If allowed: call original function, forward all return values
        let result = original_fn.call::<mlua::MultiValue>(args)?;
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::output::with_output_capture;

    #[test]
    fn test_safe_functions_work() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // Safe functions work
        let time: i64 = lua.load("return os.time()").eval().unwrap();
        assert!(time > 0);

        let date: String = lua.load("return os.date('%Y')").eval().unwrap();
        assert_eq!(date.len(), 4);

        let result: i32 = lua.load("return tonumber('42')").eval().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_safe_modules_work() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        let upper: String = lua.load("return string.upper('hello')").eval().unwrap();
        assert_eq!(upper, "HELLO");

        let sqrt: f64 = lua.load("return math.sqrt(16)").eval().unwrap();
        assert_eq!(sqrt, 4.0);

        let concat: String = lua
            .load("return table.concat({'a', 'b', 'c'}, ',')")
            .eval()
            .unwrap();
        assert_eq!(concat, "a,b,c");
    }

    #[test]
    fn test_unsafe_functions_return_nil() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // With DenyAllPolicy, unsafe functions return nil
        let result: mlua::Value = lua.load("return os.execute('echo test')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));

        let result: mlua::Value = lua.load("return io.open('test.txt', 'r')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));

        let result: mlua::Value = lua.load("return load('return 1')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_forbidden_globals_are_nil() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        let debug: mlua::Value = lua.load("return debug").eval().unwrap();
        assert!(matches!(debug, mlua::Value::Nil));

        let coroutine: mlua::Value = lua.load("return coroutine").eval().unwrap();
        assert!(matches!(coroutine, mlua::Value::Nil));

        let package: mlua::Value = lua.load("return package").eval().unwrap();
        assert!(matches!(package, mlua::Value::Nil));
    }

    #[test]
    fn test_basic_lua_functions_work() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // Test ipairs, pairs, select, etc.
        let (result, output) = with_output_capture(&lua, |lua| {
            lua.load(
                r#"
                local t = {10, 20, 30}
                for i, v in ipairs(t) do
                    print(v)
                end
            "#,
            )
            .exec()
        })
        .unwrap();

        assert!(result.is_ok());
        assert_eq!(output.len(), 3);
    }

    #[test]
    fn test_custom_globals_after_sandboxing_persist() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // Register custom function AFTER sandboxing
        lua.globals()
            .set("custom", lua.create_function(|_, ()| Ok(42)).unwrap())
            .unwrap();

        let result: i32 = lua.load("return custom()").eval().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_docs_registered() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // Verify docs table exists
        let docs_type: String = lua.load("return type(docs)").eval().unwrap();
        assert_eq!(docs_type, "table");
    }

    #[test]
    fn test_public_api_surface() {
        // Verify all public types and functions are accessible
        let lua = mlua::Lua::new();

        // 1. apply() function
        apply(&lua).unwrap();

        // 2. apply_with_policy() function
        let lua2 = mlua::Lua::new();
        let policy = Arc::new(policy::DenyAllPolicy);
        apply_with_policy(&lua2, policy, None).unwrap();

        // 3. ApiEntry type
        let _entry = ApiEntry::safe_function("test");

        // 4. SafetyLevel enum
        let _safety = SafetyLevel::Safe;

        // 5. ApiSpec type alias
        let _spec: ApiSpec = &[];

        // 6. DEFAULT_API_SPEC constant
        let _default = DEFAULT_API_SPEC;

        // 7. wrap_unsafe_function()
        lua2.load("function test() return 42 end").exec().unwrap();
        let func: mlua::Function = lua2.globals().get("test").unwrap();
        let policy2 = Arc::new(policy::DenyAllPolicy);
        let _wrapped = wrap_unsafe_function(&lua2, "test", func, policy2).unwrap();
    }
}
