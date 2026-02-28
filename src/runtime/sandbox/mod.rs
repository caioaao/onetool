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
//! For custom access control, use [`v2::apply`] directly:
//!
//! ```
//! use std::sync::Arc;
//! use onetool::runtime::sandbox;
//!
//! # fn example() -> mlua::Result<()> {
//! let lua = mlua::Lua::new();
//! let policy = Arc::new(sandbox::policy::WhiteListPolicy::new(&["os"]));
//! sandbox::v2::apply(&lua, policy, None)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Allowed Features
//!
//! - **String manipulation**: `string.*`
//! - **Table operations**: `table.*`
//! - **Math functions**: `math.*`
//! - **UTF-8 support**: `utf8.*`
//! - **Safe OS functions**: `os.time`, `os.date`
//! - **Basic operations**: `print`, `type`, `tostring`, `tonumber`, `ipairs`, `pairs`, `next`, `select`, `assert`, `error`, `pcall`, `xpcall`
//!
//! # Blocked Features
//!
//! - **File I/O**: `io.*` (wrapped, requires policy allowance)
//! - **Code loading**: `require`, `dofile`, `load`, `loadfile`, `loadstring` (wrapped)
//! - **OS commands**: `os.execute`, `os.getenv`, `os.remove`, `os.rename` (wrapped)
//! - **Metatable manipulation**: `getmetatable`, `setmetatable`, `rawset`, `rawget` (wrapped)
//! - **Forbidden entirely**: `debug`, `coroutine`, `package`

pub mod policy;
pub mod v2;

use crate::runtime::docs::{self, LuaDoc, LuaDocTyp};

/// Applies sandboxing to an existing Lua runtime using policy-based access control.
///
/// This convenience wrapper uses `DenyAllPolicy` by default, which blocks all unsafe
/// function calls. Functions are categorized as:
/// - **Safe**: No policy check, copied directly (e.g., os.time, os.date, string.*, math.*)
/// - **Unsafe**: Wrapped with policy check, returns nil on denial (e.g., os.execute, io.open)
/// - **Forbidden**: Removed entirely (debug, coroutine, package)
///
/// For custom policies, use `v2::apply()` directly.
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
    use std::sync::Arc;
    let policy = Arc::new(policy::DenyAllPolicy);
    v2::apply(lua, policy, None)?;
    register_docs(lua)?;
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

        let concat: String = lua.load("return table.concat({'a', 'b', 'c'}, ',')").eval().unwrap();
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
}
