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

use crate::runtime::docs::{self, LuaDoc, LuaDocTyp};
use crate::runtime::policy;
use std::sync::Arc;

/// Creates a package vault in the Lua registry to preserve stdlib packages.
///
/// This function must be called BEFORE sandboxing, as it captures references to
/// packages that will be set to nil during sandboxing. The vault allows
/// `onetool.require()` to restore access to these packages when permitted by
/// the access policy.
fn create_package_vault(lua: &mlua::Lua) -> mlua::Result<()> {
    let vault = lua.create_table()?;
    let globals = lua.globals();

    // Preserve io package
    if let Ok(io_pkg) = globals.get::<mlua::Value>("io") {
        vault.set("io", io_pkg)?;
    }

    // Preserve full os package (before it gets sandboxed to only time/date)
    if let Ok(os_pkg) = globals.get::<mlua::Table>("os") {
        vault.set("os", os_pkg)?;
    }

    // Preserve other potentially useful packages
    if let Ok(debug_pkg) = globals.get::<mlua::Value>("debug") {
        vault.set("debug", debug_pkg)?;
    }

    lua.set_named_registry_value("__onetool_package_vault", vault)?;
    Ok(())
}

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
    create_package_vault(lua)?;

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

/// Wraps a Lua function with policy-based access control.
///
/// Replaces the specified function in a table with a wrapper that:
/// 1. Checks the policy with `Action::CallFunction { name, args }`
/// 2. If denied: prints denial reason to stderr and returns `nil`
/// 3. If allowed: calls the original function and forwards all return values
///
/// # Arguments
/// * `lua` - The Lua environment (needed to create wrapper function)
/// * `table` - Table containing the function (e.g., `lua.globals()` or an `os` table)
/// * `function_name` - Key in the table (e.g., `"execute"` for `os.execute`)
/// * `policy` - Thread-safe policy reference (`Arc<P>` for sharing across closures)
///
/// # Example
/// ```
/// use std::sync::Arc;
/// use onetool::runtime::{sandbox, policy};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let policy = Arc::new(policy::DenyAllPolicy);
///
/// // Wrap a global function
/// sandbox::wrap_unsafe_call(&lua, &lua.globals(), "dofile", policy.clone())?;
///
/// // Wrap a module function
/// let os_table = lua.globals().get::<mlua::Table>("os")?;
/// sandbox::wrap_unsafe_call(&lua, &os_table, "execute", policy)?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Returns `mlua::Error` if:
/// - The function does not exist in the table
/// - The value at `function_name` is not a function
pub fn wrap_unsafe_call<P: policy::Policy + 'static>(
    lua: &mlua::Lua,
    table: &mlua::Table,
    function_name: &str,
    policy: Arc<P>,
) -> mlua::Result<()> {
    // 1. Get original function (fails if missing or not a function)
    let original_fn: mlua::Function = table.get(function_name)?;

    // 2. Create wrapper function that captures policy and original function
    let function_name_owned = function_name.to_string();
    let wrapper = lua.create_function(move |_lua, args: mlua::MultiValue| {
        // Check policy with CallFunction action
        let action = policy::Action::CallFunction {
            name: function_name_owned.clone(),
            args: args.clone(),
        };

        let decision = policy.check_access(&policy::Caller::Agent, &action);

        // If denied: print reason to stderr, return nil as MultiValue
        if let policy::AccessDecision::Deny(reason) = decision {
            eprintln!("Access denied: {}", reason);
            return Ok(mlua::MultiValue::from_vec(vec![mlua::Value::Nil]));
        }

        // If allowed: call original function, forward all return values
        let result = original_fn.call::<mlua::MultiValue>(args)?;
        Ok(result)
    })?;

    // 3. Replace function in table
    table.set(function_name, wrapper)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::runtime::output::with_output_capture;
    use crate::runtime::policy::{AccessDecision, Action, Caller, Policy};

    use super::*;

    // ============================================================================
    // Test Helper Policies
    // ============================================================================

    /// Test policy that always allows access
    struct AllowPolicy;

    impl Policy for AllowPolicy {
        fn check_access(&self, _: &Caller, _: &Action) -> AccessDecision {
            AccessDecision::Allow
        }
    }

    /// Test policy that always denies access
    struct DenyPolicy;

    impl Policy for DenyPolicy {
        fn check_access(&self, _: &Caller, _: &Action) -> AccessDecision {
            AccessDecision::Deny("test denial".to_string())
        }
    }

    /// Test policy that captures the action for verification
    struct CapturingPolicy {
        captured_name: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        captured_args_count: std::sync::Arc<std::sync::Mutex<Option<usize>>>,
    }

    impl CapturingPolicy {
        fn new() -> Self {
            Self {
                captured_name: std::sync::Arc::new(std::sync::Mutex::new(None)),
                captured_args_count: std::sync::Arc::new(std::sync::Mutex::new(None)),
            }
        }

        fn get_captured_name(&self) -> Option<String> {
            self.captured_name.lock().unwrap().clone()
        }

        fn get_captured_args_count(&self) -> Option<usize> {
            *self.captured_args_count.lock().unwrap()
        }
    }

    impl Policy for CapturingPolicy {
        fn check_access(&self, _: &Caller, action: &Action) -> AccessDecision {
            if let Action::CallFunction { name, args } = action {
                *self.captured_name.lock().unwrap() = Some(name.clone());
                *self.captured_args_count.lock().unwrap() = Some(args.len());
            }
            AccessDecision::Allow
        }
    }

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

    // ============================================================================
    // wrap_unsafe_call Tests
    // ============================================================================

    #[test]
    fn wrap_basic_function_allowed() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a simple test function
        lua.load(
            r#"
            function test_func(x)
                return x * 2
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it with allow policy
        wrap_unsafe_call(&lua, &lua.globals(), "test_func", policy).unwrap();

        // Verify it still works
        let result: i32 = lua.load("return test_func(5)").eval().unwrap();
        assert_eq!(result, 10);
    }

    #[test]
    fn wrap_function_policy_denial() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);

        // Create a test function
        lua.load(
            r#"
            function dangerous_func()
                return "should not execute"
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it with deny policy
        wrap_unsafe_call(&lua, &lua.globals(), "dangerous_func", policy).unwrap();

        // Verify it returns nil
        let result: mlua::Value = lua.load("return dangerous_func()").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn wrap_nonexistent_function() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Try to wrap a function that doesn't exist
        let result = wrap_unsafe_call(&lua, &lua.globals(), "nonexistent", policy);

        // Should return an error
        assert!(result.is_err());
    }

    #[test]
    fn wrap_non_function_value() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Set a non-function value
        lua.globals().set("not_a_func", 42).unwrap();

        // Try to wrap it
        let result = wrap_unsafe_call(&lua, &lua.globals(), "not_a_func", policy);

        // Should return an error
        assert!(result.is_err());
    }

    #[test]
    fn wrap_preserves_multiple_return_values() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that returns multiple values
        lua.load(
            r#"
            function multi_return(a, b, c)
                return a, b, c
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it
        wrap_unsafe_call(&lua, &lua.globals(), "multi_return", policy).unwrap();

        // Verify all return values are preserved
        let result: (i32, i32, i32) = lua.load("return multi_return(1, 2, 3)").eval().unwrap();
        assert_eq!(result, (1, 2, 3));
    }

    #[test]
    fn wrap_function_no_arguments() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function with no args
        lua.load(
            r#"
            function no_args()
                return 42
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it
        wrap_unsafe_call(&lua, &lua.globals(), "no_args", policy).unwrap();

        // Verify it works
        let result: i32 = lua.load("return no_args()").eval().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn wrap_function_many_arguments() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a variadic function
        lua.load(
            r#"
            function sum(...)
                local total = 0
                for _, v in ipairs({...}) do
                    total = total + v
                end
                return total
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it
        wrap_unsafe_call(&lua, &lua.globals(), "sum", policy).unwrap();

        // Verify it works with many arguments
        let result: i32 = lua.load("return sum(1, 2, 3, 4, 5)").eval().unwrap();
        assert_eq!(result, 15);
    }

    #[test]
    fn wrap_error_propagation() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that errors
        lua.load(
            r#"
            function error_func()
                error("intentional error")
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it
        wrap_unsafe_call(&lua, &lua.globals(), "error_func", policy).unwrap();

        // Verify error propagates
        let result = lua.load("return error_func()").exec();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("intentional error")
        );
    }

    #[test]
    fn wrap_table_member_function() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a table with a function
        lua.load(
            r#"
            my_table = {
                func = function(x)
                    return x + 10
                end
            }
        "#,
        )
        .exec()
        .unwrap();

        // Get the table and wrap its function
        let my_table: mlua::Table = lua.globals().get("my_table").unwrap();
        wrap_unsafe_call(&lua, &my_table, "func", policy).unwrap();

        // Verify it works
        let result: i32 = lua.load("return my_table.func(5)").eval().unwrap();
        assert_eq!(result, 15);
    }

    #[test]
    fn wrap_policy_receives_function_name() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(CapturingPolicy::new());
        let policy_clone = policy.clone();

        // Create a test function
        lua.load(
            r#"
            function my_func()
                return 42
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it
        wrap_unsafe_call(&lua, &lua.globals(), "my_func", policy).unwrap();

        // Call the wrapped function
        let _result: i32 = lua.load("return my_func()").eval().unwrap();

        // Verify policy received the correct function name
        assert_eq!(
            policy_clone.get_captured_name(),
            Some("my_func".to_string())
        );
    }

    #[test]
    fn wrap_policy_receives_args() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(CapturingPolicy::new());
        let policy_clone = policy.clone();

        // Create a test function
        lua.load(
            r#"
            function count_args(a, b, c)
                return 3
            end
        "#,
        )
        .exec()
        .unwrap();

        // Wrap it
        wrap_unsafe_call(&lua, &lua.globals(), "count_args", policy).unwrap();

        // Call with 3 arguments
        let _result: i32 = lua.load("return count_args(1, 2, 3)").eval().unwrap();

        // Verify policy received 3 arguments
        assert_eq!(policy_clone.get_captured_args_count(), Some(3));
    }
}
