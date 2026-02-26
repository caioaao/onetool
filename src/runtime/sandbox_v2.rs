//! Lua runtime sandboxing (v2 API - functional approach).
//!
//! This module provides an alternative sandboxing API that operates on function values
//! rather than mutating tables in-place. This enables functional composition patterns
//! and gives more control over where wrapped functions are assigned.
//!
//! # Comparison with v1 API
//!
//! - **v1 ([`sandbox::wrap_unsafe_call`])**: Mutates a table by replacing a function key
//! - **v2 ([`wrap_unsafe_function`])**: Returns a new wrapped function value
//!
//! Use v1 for directly replacing global functions or module functions.
//! Use v2 when you need to compose wrapped functions or create utility tables.
//!
//! [`sandbox::wrap_unsafe_call`]: super::sandbox::wrap_unsafe_call
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use onetool::runtime::{sandbox_v2, policy};
//!
//! # fn example() -> mlua::Result<()> {
//! let lua = mlua::Lua::new();
//! let policy = Arc::new(policy::DenyAllPolicy);
//!
//! // Get original function
//! let os_table: mlua::Table = lua.globals().get("os")?;
//! let execute: mlua::Function = os_table.get("execute")?;
//!
//! // Wrap it without mutating the table
//! let wrapped = sandbox_v2::wrap_unsafe_function(&lua, "os.execute", execute, policy)?;
//!
//! // Assign wherever needed
//! lua.globals().set("safe_execute", wrapped)?;
//! # Ok(())
//! # }
//! ```

use super::policy;
use std::sync::Arc;

/// Creates a policy-controlled wrapper around a Lua function.
///
/// Returns a new function that intercepts calls, checks the policy, and either
/// forwards to the original function or returns `nil` on denial. Unlike
/// [`wrap_unsafe_call`], this does not mutate any tables - it returns a new
/// function value that can be assigned wherever needed.
///
/// [`wrap_unsafe_call`]: super::sandbox::wrap_unsafe_call
///
/// # Arguments
///
/// * `lua` - The Lua environment (needed to create the wrapper closure)
/// * `function_name` - Name used in policy checks and denial logs (not for lookup)
/// * `original_fn` - The Lua function to wrap
/// * `policy` - Thread-safe policy reference (`Arc<P>`) for access decisions
///
/// # Returns
///
/// A new wrapped function that:
/// - Checks policy with `Action::CallFunction { name, args }` before each call
/// - Returns `nil` (single value) when denied, logging reason to stderr
/// - Forwards all return values from `original_fn` when allowed
/// - Propagates any Lua errors raised by `original_fn`
///
/// # Behavior Details
///
/// **Policy checking:**
/// - Always uses `Caller::Agent` as the caller identity
/// - Function name is passed as provided (not resolved from environment)
/// - All arguments are cloned and passed to the policy for inspection
///
/// **Denial handling:**
/// - Logs denial reason to stderr (visible to users)
/// - Returns `MultiValue::from_vec(vec![Value::Nil])` (single nil)
/// - Does not raise a Lua error
///
/// **Allow handling:**
/// - Calls original function with exact arguments received
/// - Forwards all return values (preserves multiple returns)
/// - Propagates any errors from the original function
///
/// # Comparison: `wrap_unsafe_call` vs `wrap_unsafe_function`
///
/// | Feature              | `wrap_unsafe_call`  | `wrap_unsafe_function` |
/// |----------------------|---------------------|------------------------|
/// | Mutates table        | Yes                 | No                     |
/// | Returns              | `()`                | `Function`             |
/// | Use case             | Replace globals     | Composition            |
/// | Error on missing fn  | Yes                 | N/A (fn is parameter)  |
///
/// # Security Considerations
///
/// - **Always uses `Caller::Agent`**: Does not distinguish between different callers
/// - **Denial logging**: Denials are logged to stderr, visible to users
/// - **Original function errors**: Lua errors from the original function still propagate
/// - **Direct references**: If other references to `original_fn` exist, they bypass the wrapper
///
/// # Examples
///
/// ## Basic wrapping with allow policy
///
/// ```
/// use std::sync::Arc;
/// use onetool::runtime::{sandbox_v2, policy};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let policy = Arc::new(policy::WhiteListPolicy::new(&["io"]));
///
/// // Create a simple Lua function
/// lua.load("function add(a, b) return a + b end").exec()?;
/// let add_fn: mlua::Function = lua.globals().get("add")?;
///
/// // Wrap it (policy allows all function calls in this example)
/// let wrapped = sandbox_v2::wrap_unsafe_function(&lua, "add", add_fn, policy)?;
///
/// // Assign to a new name
/// lua.globals().set("safe_add", wrapped)?;
///
/// // Call works and returns correct result
/// let result: i32 = lua.load("return safe_add(3, 5)").eval()?;
/// assert_eq!(result, 8);
/// # Ok(())
/// # }
/// ```
///
/// ## Denial scenario
///
/// ```
/// use std::sync::Arc;
/// use onetool::runtime::{sandbox_v2, policy};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let deny_policy = Arc::new(policy::DenyAllPolicy);
///
/// // Get os.execute
/// let os_table: mlua::Table = lua.globals().get("os")?;
/// let execute: mlua::Function = os_table.get("execute")?;
///
/// // Wrap with deny policy
/// let wrapped = sandbox_v2::wrap_unsafe_function(&lua, "os.execute", execute, deny_policy)?;
/// lua.globals().set("safe_execute", wrapped)?;
///
/// // Call returns nil (denial logged to stderr)
/// let result: mlua::Value = lua.load("return safe_execute('ls')").eval()?;
/// assert!(matches!(result, mlua::Value::Nil));
/// # Ok(())
/// # }
/// ```
///
/// ## Functional composition - utility table
///
/// ```
/// use std::sync::Arc;
/// use onetool::runtime::{sandbox_v2, policy};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let policy = Arc::new(policy::WhiteListPolicy::new(&["os"]));
///
/// // Create a utility table with multiple wrapped functions
/// let os_table: mlua::Table = lua.globals().get("os")?;
/// let time_fn: mlua::Function = os_table.get("time")?;
/// let date_fn: mlua::Function = os_table.get("date")?;
///
/// let wrapped_time = sandbox_v2::wrap_unsafe_function(&lua, "os.time", time_fn, policy.clone())?;
/// let wrapped_date = sandbox_v2::wrap_unsafe_function(&lua, "os.date", date_fn, policy)?;
///
/// let utils = lua.create_table()?;
/// utils.set("time", wrapped_time)?;
/// utils.set("date", wrapped_date)?;
/// lua.globals().set("safe_os", utils)?;
///
/// // Use the utility table
/// let timestamp: i64 = lua.load("return safe_os.time()").eval()?;
/// assert!(timestamp > 0);
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns `mlua::Error` if:
/// - Creating the wrapper function fails (invalid Lua state)
///
/// Note: Wrapped function calls can also error if:
/// - The original function raises a Lua error (propagated)
/// - Policy denials do NOT raise errors (they return `nil` instead)
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

        let decision = policy.check_access(&policy::Caller::Agent, &action);

        // If denied: print reason to stderr, return nil as MultiValue
        if let policy::AccessDecision::Deny(reason) = decision {
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
    use crate::runtime::policy::{AccessDecision, Action, Caller, Policy};
    use std::sync::{Arc, Mutex};

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
        captured_name: Arc<Mutex<Option<String>>>,
        captured_args_count: Arc<Mutex<Option<usize>>>,
        captured_caller: Arc<Mutex<Option<Caller>>>,
    }

    impl CapturingPolicy {
        fn new() -> Self {
            Self {
                captured_name: Arc::new(Mutex::new(None)),
                captured_args_count: Arc::new(Mutex::new(None)),
                captured_caller: Arc::new(Mutex::new(None)),
            }
        }

        fn get_captured_name(&self) -> Option<String> {
            self.captured_name.lock().unwrap().clone()
        }

        fn get_captured_args_count(&self) -> Option<usize> {
            *self.captured_args_count.lock().unwrap()
        }

        fn get_captured_caller(&self) -> Option<Caller> {
            self.captured_caller.lock().unwrap().clone()
        }
    }

    impl Policy for CapturingPolicy {
        fn check_access(&self, caller: &Caller, action: &Action) -> AccessDecision {
            if let Action::CallFunction { name, args } = action {
                *self.captured_name.lock().unwrap() = Some(name.clone());
                *self.captured_args_count.lock().unwrap() = Some(args.len());
                *self.captured_caller.lock().unwrap() = Some(caller.clone());
            }
            AccessDecision::Allow
        }
    }

    // ============================================================================
    // wrap_unsafe_function Tests
    // ============================================================================

    // Basic happy path

    #[test]
    fn test_wrap_function_allowed_basic() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a simple Lua function
        lua.load("function double(x) return x * 2 end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("double").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "double", original, policy).unwrap();
        lua.globals().set("wrapped_double", wrapped).unwrap();

        // Call and verify result
        let result: i32 = lua.load("return wrapped_double(5)").eval().unwrap();
        assert_eq!(result, 10);
    }

    #[test]
    fn test_wrap_function_no_args() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function with no arguments
        lua.load("function get_answer() return 42 end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("get_answer").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "get_answer", original, policy).unwrap();
        lua.globals().set("wrapped_answer", wrapped).unwrap();

        // Call and verify
        let result: i32 = lua.load("return wrapped_answer()").eval().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_wrap_function_single_arg() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function with single arg
        lua.load("function increment(x) return x + 1 end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("increment").unwrap();

        // Wrap and test
        let wrapped = wrap_unsafe_function(&lua, "increment", original, policy).unwrap();
        lua.globals().set("safe_inc", wrapped).unwrap();

        let result: i32 = lua.load("return safe_inc(10)").eval().unwrap();
        assert_eq!(result, 11);
    }

    // Policy denial

    #[test]
    fn test_wrap_function_denied_returns_nil() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);

        // Create a function
        lua.load("function dangerous() return 'should not see this' end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("dangerous").unwrap();

        // Wrap with deny policy
        let wrapped = wrap_unsafe_function(&lua, "dangerous", original, policy).unwrap();
        lua.globals().set("wrapped_dangerous", wrapped).unwrap();

        // Call should return nil
        let result: mlua::Value = lua.load("return wrapped_dangerous()").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    // Multiple arguments

    #[test]
    fn test_wrap_function_multiple_args() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function with multiple args
        lua.load("function add(a, b, c) return a + b + c end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("add").unwrap();

        // Wrap and test
        let wrapped = wrap_unsafe_function(&lua, "add", original, policy).unwrap();
        lua.globals().set("safe_add", wrapped).unwrap();

        let result: i32 = lua.load("return safe_add(1, 2, 3)").eval().unwrap();
        assert_eq!(result, 6);
    }

    #[test]
    fn test_wrap_function_variadic_lua_function() {
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
        let original: mlua::Function = lua.globals().get("sum").unwrap();

        // Wrap and test with many args
        let wrapped = wrap_unsafe_function(&lua, "sum", original, policy).unwrap();
        lua.globals().set("safe_sum", wrapped).unwrap();

        let result: i32 = lua.load("return safe_sum(1, 2, 3, 4, 5)").eval().unwrap();
        assert_eq!(result, 15);
    }

    // Multiple return values

    #[test]
    fn test_wrap_function_multiple_returns() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that returns multiple values
        lua.load("function multi() return 1, 2, 3 end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("multi").unwrap();

        // Wrap and test
        let wrapped = wrap_unsafe_function(&lua, "multi", original, policy).unwrap();
        lua.globals().set("safe_multi", wrapped).unwrap();

        let result: (i32, i32, i32) = lua.load("return safe_multi()").eval().unwrap();
        assert_eq!(result, (1, 2, 3));
    }

    #[test]
    fn test_wrap_function_zero_returns() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that returns nothing
        lua.load("function nothing() end").exec().unwrap();
        let original: mlua::Function = lua.globals().get("nothing").unwrap();

        // Wrap and test
        let wrapped = wrap_unsafe_function(&lua, "nothing", original, policy).unwrap();
        lua.globals().set("safe_nothing", wrapped).unwrap();

        // Should return nil when function returns nothing
        let result: mlua::Value = lua.load("return safe_nothing()").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_wrap_function_nil_return() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that explicitly returns nil
        lua.load("function ret_nil() return nil end").exec().unwrap();
        let original: mlua::Function = lua.globals().get("ret_nil").unwrap();

        // Wrap and test
        let wrapped = wrap_unsafe_function(&lua, "ret_nil", original, policy).unwrap();
        lua.globals().set("safe_nil", wrapped).unwrap();

        let result: mlua::Value = lua.load("return safe_nil()").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    // Error handling

    #[test]
    fn test_wrap_function_error_propagates() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that errors
        lua.load("function boom() error('explosion!') end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("boom").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "boom", original, policy).unwrap();
        lua.globals().set("safe_boom", wrapped).unwrap();

        // Error should propagate
        let result = lua.load("return safe_boom()").exec();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("explosion!"));
    }

    #[test]
    fn test_wrap_function_allowed_call_can_error() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that can error conditionally
        lua.load(
            r#"
            function conditional_error(should_error)
                if should_error then
                    error("conditional error")
                end
                return "ok"
            end
        "#,
        )
        .exec()
        .unwrap();
        let original: mlua::Function = lua.globals().get("conditional_error").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "conditional_error", original, policy).unwrap();
        lua.globals().set("safe_conditional", wrapped).unwrap();

        // Should work when not erroring
        let result: String = lua.load("return safe_conditional(false)").eval().unwrap();
        assert_eq!(result, "ok");

        // Should propagate error when erroring
        let result = lua.load("return safe_conditional(true)").exec();
        assert!(result.is_err());
    }

    // Edge cases

    #[test]
    fn test_wrap_function_nil_argument() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that handles nil
        lua.load("function handle_nil(x) return x == nil end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("handle_nil").unwrap();

        // Wrap and test with nil argument
        let wrapped = wrap_unsafe_function(&lua, "handle_nil", original, policy).unwrap();
        lua.globals().set("safe_handle_nil", wrapped).unwrap();

        let result: bool = lua.load("return safe_handle_nil(nil)").eval().unwrap();
        assert!(result);
    }

    #[test]
    fn test_wrap_function_mixed_types() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function that takes mixed types
        lua.load(
            r#"
            function mixed(str, num, bool, tbl)
                return type(str), type(num), type(bool), type(tbl)
            end
        "#,
        )
        .exec()
        .unwrap();
        let original: mlua::Function = lua.globals().get("mixed").unwrap();

        // Wrap and test
        let wrapped = wrap_unsafe_function(&lua, "mixed", original, policy).unwrap();
        lua.globals().set("safe_mixed", wrapped).unwrap();

        let result: (String, String, String, String) = lua
            .load("return safe_mixed('hello', 42, true, {})")
            .eval()
            .unwrap();
        assert_eq!(result, ("string".to_string(), "number".to_string(), "boolean".to_string(), "table".to_string()));
    }

    // Policy interaction

    #[test]
    fn test_wrap_function_policy_receives_correct_name() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(CapturingPolicy::new());
        let policy_clone = policy.clone();

        // Create a function
        lua.load("function my_func() return 1 end").exec().unwrap();
        let original: mlua::Function = lua.globals().get("my_func").unwrap();

        // Wrap with custom name
        let wrapped = wrap_unsafe_function(&lua, "custom_name", original, policy).unwrap();
        lua.globals().set("wrapped", wrapped).unwrap();

        // Call it
        let _: i32 = lua.load("return wrapped()").eval().unwrap();

        // Verify policy received the custom name
        assert_eq!(
            policy_clone.get_captured_name(),
            Some("custom_name".to_string())
        );
    }

    #[test]
    fn test_wrap_function_policy_receives_args() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(CapturingPolicy::new());
        let policy_clone = policy.clone();

        // Create a function
        lua.load("function count_args(...) return select('#', ...) end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("count_args").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "count_args", original, policy).unwrap();
        lua.globals().set("wrapped", wrapped).unwrap();

        // Call with 4 arguments
        let _: i32 = lua.load("return wrapped(1, 2, 3, 4)").eval().unwrap();

        // Verify policy received 4 arguments
        assert_eq!(policy_clone.get_captured_args_count(), Some(4));
    }

    #[test]
    fn test_wrap_function_uses_agent_caller() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(CapturingPolicy::new());
        let policy_clone = policy.clone();

        // Create a function
        lua.load("function test() return 1 end").exec().unwrap();
        let original: mlua::Function = lua.globals().get("test").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "test", original, policy).unwrap();
        lua.globals().set("wrapped", wrapped).unwrap();

        // Call it
        let _: i32 = lua.load("return wrapped()").eval().unwrap();

        // Verify policy received Caller::Agent
        assert_eq!(
            policy_clone.get_captured_caller(),
            Some(Caller::Agent)
        );
    }

    // Integration

    #[test]
    fn test_wrap_function_assign_to_table() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create a function
        lua.load("function original() return 'works' end")
            .exec()
            .unwrap();
        let original: mlua::Function = lua.globals().get("original").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "original", original, policy).unwrap();

        // Create a table and assign wrapped function to it
        let my_table = lua.create_table().unwrap();
        my_table.set("func", wrapped).unwrap();
        lua.globals().set("my_table", my_table).unwrap();

        // Call via table
        let result: String = lua.load("return my_table.func()").eval().unwrap();
        assert_eq!(result, "works");
    }

    #[test]
    fn test_wrap_function_assign_to_global() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Get os.time
        let os_table: mlua::Table = lua.globals().get("os").unwrap();
        let time_fn: mlua::Function = os_table.get("time").unwrap();

        // Wrap it
        let wrapped = wrap_unsafe_function(&lua, "os.time", time_fn, policy).unwrap();

        // Assign to global
        lua.globals().set("safe_time", wrapped).unwrap();

        // Call and verify it returns a number
        let result: i64 = lua.load("return safe_time()").eval().unwrap();
        assert!(result > 0);
    }
}
