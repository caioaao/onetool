//! Lua runtime sandboxing.
//!
//! This module implements security restrictions for Lua runtime environments:
//! - **[`apply`]**: Sets dangerous Lua globals to `nil` (blocks all access)
//! - **[`wrap_unsafe_call`]**: Wraps functions with policy-based access control (conditional access)
//!
//! Attempts to use blocked features will fail with "attempt to call a nil value" errors.
//!
//! # Blocked Features (by [`apply`])
//!
//! - **File I/O**: `io`, `file`
//! - **Code loading**: `require`, `dofile`, `load`, `loadfile`, `loadstring`, `package`
//! - **OS commands**: `os.execute`, `os.getenv`, `os.remove`, `os.rename`, etc.
//! - **Metatable manipulation**: `getmetatable`, `setmetatable`, `rawset`, `rawget`, `rawequal`, `rawlen`
//! - **Memory control**: `collectgarbage`
//! - **Coroutines**: `coroutine`
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
//! # Example
//!
//! ```
//! use onetool::runtime::sandbox;
//!
//! # fn example() -> mlua::Result<()> {
//! let lua = mlua::Lua::new();
//! sandbox::apply(&lua)?;
//!
//! // This will fail
//! let result = lua.load("io.open('test.txt')").exec();
//! assert!(result.is_err());
//! # Ok(())
//! # }
//! ```

pub mod policy;
pub mod v2;

use crate::runtime::docs::{self, LuaDoc, LuaDocTyp};

/// Applies sandboxing to an existing Lua runtime.
///
/// Sandboxed packages are added to a `vault` so it can be accessed by priviledged actors
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
/// # Ok(())
/// # }
/// ```
pub fn apply(lua: &mlua::Lua) -> mlua::Result<()> {
    // First, preserve safe os functions before blocking
    sandbox_os_module(lua)?;

    let globals = lua.globals();

    // File I/O
    globals.set("io", mlua::Value::Nil)?;
    globals.set("file", mlua::Value::Nil)?;

    // Code loading
    globals.set("require", mlua::Value::Nil)?;
    globals.set("dofile", mlua::Value::Nil)?;
    globals.set("load", mlua::Value::Nil)?;
    globals.set("loadfile", mlua::Value::Nil)?;
    globals.set("loadstring", mlua::Value::Nil)?;
    globals.set("package", mlua::Value::Nil)?;

    // Debug/introspection (Lua::new() already excludes debug, but be explicit)
    globals.set("debug", mlua::Value::Nil)?;
    globals.set("rawget", mlua::Value::Nil)?;
    globals.set("rawset", mlua::Value::Nil)?;
    globals.set("rawequal", mlua::Value::Nil)?;
    globals.set("rawlen", mlua::Value::Nil)?;
    globals.set("getmetatable", mlua::Value::Nil)?;
    globals.set("setmetatable", mlua::Value::Nil)?;

    // Memory control
    globals.set("collectgarbage", mlua::Value::Nil)?;

    // Coroutines
    globals.set("coroutine", mlua::Value::Nil)?;

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

fn sandbox_os_module(lua: &mlua::Lua) -> mlua::Result<()> {
    let globals = lua.globals();
    let os_table: mlua::Table = globals.get("os")?;

    // Extract safe functions before removing the module
    let os_time: mlua::Function = os_table.get("time")?;
    let os_date: mlua::Function = os_table.get("date")?;

    // Create restricted os table with only safe functions
    let safe_os = lua.create_table()?;
    safe_os.set("time", os_time)?;
    safe_os.set("date", os_date)?;

    globals.set("os", safe_os)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::runtime::output::with_output_capture;

    use super::*;

    #[test]
    fn blocked_io_module() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // Accessing io.open should fail since io is set to nil
        let result = lua.load("local f = io.open('test.txt', 'r')").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_os_execute() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // os.execute should not exist in sandboxed os table
        let result = lua.load("os.execute('ls')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn blocked_os_getenv() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // os.getenv should not exist in sandboxed os table
        let result = lua.load("os.getenv('PATH')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn allowed_os_time() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // os.time should work
        let (result, output) =
            with_output_capture(&lua, |lua| lua.load("print(type(os.time()))").exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output.len(), 1);
        assert!(output[0].contains("number"));
    }

    #[test]
    fn allowed_os_date() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // os.date should work
        let (result, output) = with_output_capture(&lua, |lua| {
            lua.load("print(type(os.date('%Y-%m-%d')))").exec()
        })
        .unwrap();

        assert!(result.is_ok());
        assert_eq!(output.len(), 1);
        assert!(output[0].contains("string"));
    }

    #[test]
    fn blocked_require() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // require should be nil
        let result = lua.load("require('os')").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_load() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // load should be nil
        let result = lua.load("load('print(1)')()").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_dofile() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // dofile should be nil
        let result = lua.load("dofile('/etc/passwd')").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_collectgarbage() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // collectgarbage should be nil
        let result = lua.load("collectgarbage('collect')").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_coroutine() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // coroutine should be nil
        let result = lua.load("coroutine.create(function() end)").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_rawset() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // rawset should be nil
        let result = lua.load("rawset(_G, 'x', 1)").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }

    #[test]
    fn blocked_getmetatable() {
        let lua = mlua::Lua::new();
        apply(&lua).unwrap();

        // getmetatable should be nil
        let result = lua.load("getmetatable('')").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nil"));
    }
}
