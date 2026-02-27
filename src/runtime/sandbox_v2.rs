//! Lua runtime sandboxing (v2 API - functional approach).

use super::policy;
use std::sync::Arc;

// ============================================================================
// API Specification (Compile-time Constants)
// ============================================================================

/// Safety level for a Lua function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    /// Function is safe - no policy check, copied directly
    Safe,
    /// Function requires policy check - wrapped with access control
    Unsafe,
    // Note: Functions NOT in the spec are implicitly Forbidden (removed)
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
    // Global functions (top-level)
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
    ApiEntry::safe_function("type"),
    ApiEntry::safe_function("tonumber"),
    ApiEntry::safe_function("tostring"),
];

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
        }
    }

    Ok(target_table)
}

/// Applies sandboxing with policy-based access control
///
/// # Arguments
/// * `lua` - The Lua environment
/// * `policy` - Policy for access control decisions
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
/// use onetool::runtime::{sandbox_v2, policy};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let policy = Arc::new(policy::DenyAllPolicy);
/// sandbox_v2::apply(&lua, policy, None)?; // Use default spec
///
/// // os.execute is wrapped - returns nil on denial
/// let result: mlua::Value = lua.load("return os.execute('echo test')").eval()?;
/// assert!(matches!(result, mlua::Value::Nil));
///
/// // os.time is safe - works without policy checks
/// let timestamp: i64 = lua.load("return os.time()").eval()?;
/// assert!(timestamp > 0);
/// # Ok(())
/// # }
/// ```
pub fn apply<P: policy::Policy + 'static>(
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
        lua.load("function ret_nil() return nil end")
            .exec()
            .unwrap();
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
        assert_eq!(
            result,
            (
                "string".to_string(),
                "number".to_string(),
                "boolean".to_string(),
                "table".to_string()
            )
        );
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
        assert_eq!(policy_clone.get_captured_caller(), Some(Caller::Agent));
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

    // ============================================================================
    // apply() function tests
    // ============================================================================

    #[test]
    fn test_apply_os_execute_wrapped_deny_policy() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // Should be wrapped (returns nil on denial), not blocked (error)
        let result: mlua::Value = lua.load("return os.execute('echo test')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_os_time_not_wrapped() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // os.time should work without policy checks (it's in the safe list)
        let result: i64 = lua.load("return os.time()").eval().unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_apply_os_date_not_wrapped() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // os.date should work without policy checks (it's in the safe list)
        let result: String = lua.load("return os.date('%Y')").eval().unwrap();
        // Should return a 4-digit year
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_apply_io_open_wrapped_deny_policy() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // io.open should be wrapped and return nil on denial
        let result: mlua::Value = lua.load("return io.open('test.txt', 'r')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_global_load_wrapped_deny_policy() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // load should be wrapped and return nil on denial
        let result: mlua::Value = lua.load("return load('return 1')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_global_require_wrapped_deny_policy() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // require should be wrapped and return nil on denial
        let result: mlua::Value = lua.load("return require('os')").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_debug_blocked_completely() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        apply(&lua, policy, None).unwrap();

        // debug should be nil (completely blocked, not wrapped)
        let result = lua.load("return debug").eval::<mlua::Value>().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_coroutine_blocked_completely() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        apply(&lua, policy, None).unwrap();

        // coroutine should be nil (completely blocked, not wrapped)
        let result = lua.load("return coroutine").eval::<mlua::Value>().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_package_blocked_completely() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        apply(&lua, policy, None).unwrap();

        // package should be nil (completely blocked, not wrapped)
        let result = lua.load("return package").eval::<mlua::Value>().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_with_allow_policy_os_execute_works() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        apply(&lua, policy, None).unwrap();

        // With AllowPolicy, os.execute should work (though we can't test actual execution)
        // At minimum, it should not return nil due to policy denial
        let result = lua
            .load("return type(os.execute)")
            .eval::<String>()
            .unwrap();
        assert_eq!(result, "function");
    }

    #[test]
    fn test_apply_qualified_names_used() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(CapturingPolicy::new());
        let policy_clone = policy.clone();

        apply(&lua, policy, None).unwrap();

        // Call os.execute and verify the policy received the qualified name
        let _: mlua::Value = lua.load("return os.execute('echo test')").eval().unwrap();

        // Verify policy received "os.execute", not just "execute"
        assert_eq!(
            policy_clone.get_captured_name(),
            Some("os.execute".to_string())
        );
    }

    #[test]
    fn test_apply_os_functions_wrapped_or_safe() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // Verify safe functions work directly (not wrapped)
        let time_result: i64 = lua.load("return os.time()").eval().unwrap();
        assert!(time_result > 0);

        let date_result: String = lua.load("return os.date('%Y')").eval().unwrap();
        assert_eq!(date_result.len(), 4);

        // Verify unsafe functions are present but wrapped (return nil with DenyPolicy)
        // os.execute exists but is wrapped
        let execute_type: String = lua.load("return type(os.execute)").eval().unwrap();
        assert_eq!(execute_type, "function");

        // But calling it returns nil due to policy denial
        let execute_result: mlua::Value =
            lua.load("return os.execute('echo test')").eval().unwrap();
        assert!(matches!(execute_result, mlua::Value::Nil));
    }

    #[test]
    fn test_apply_safe_globals_still_work() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);
        apply(&lua, policy, None).unwrap();

        // Safe globals should still work
        let result: i32 = lua.load("return tonumber('42')").eval().unwrap();
        assert_eq!(result, 42);

        let result: String = lua.load("return type(42)").eval().unwrap();
        assert_eq!(result, "number");

        let result: String = lua.load("return tostring(42)").eval().unwrap();
        assert_eq!(result, "42");
    }

    // ============================================================================
    // Custom API Spec Tests
    // ============================================================================

    #[test]
    fn test_apply_with_custom_spec() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);

        // Create minimal custom spec
        const CUSTOM_SPEC: ApiSpec = &[ApiEntry::Module {
            name: "os",
            entries: &[ApiEntry::Function {
                name: "time",
                safety: SafetyLevel::Safe,
            }],
        }];

        apply(&lua, policy, Some(CUSTOM_SPEC)).unwrap();

        // Only os.time should exist
        let time: i64 = lua.load("return os.time()").eval().unwrap();
        assert!(time > 0);

        // os.date not in spec = forbidden (should be nil or error)
        let result = lua.load("return os.date").eval::<mlua::Value>();
        // Either it's nil or we get an error trying to access it
        match result {
            Ok(mlua::Value::Nil) => { /* expected */ }
            Err(_) => { /* also acceptable */ }
            Ok(_) => panic!("os.date should not exist in custom spec"),
        }
    }

    #[test]
    fn test_apply_with_empty_spec_forbids_everything() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Empty spec = everything forbidden
        const EMPTY_SPEC: ApiSpec = &[];
        apply(&lua, policy, Some(EMPTY_SPEC)).unwrap();

        // os module not in spec = forbidden
        let result = lua.load("return os").eval::<mlua::Value>().unwrap();
        assert!(matches!(result, mlua::Value::Nil));

        // io module not in spec = forbidden
        let result = lua.load("return io").eval::<mlua::Value>().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_custom_spec_with_unsafe_functions() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Custom spec with unsafe function
        const CUSTOM_SPEC: ApiSpec = &[
            ApiEntry::Module {
                name: "os",
                entries: &[ApiEntry::Function {
                    name: "execute",
                    safety: SafetyLevel::Unsafe,
                }],
            },
            ApiEntry::Function {
                name: "type",
                safety: SafetyLevel::Safe,
            },
        ];

        apply(&lua, policy, Some(CUSTOM_SPEC)).unwrap();

        // os.execute should exist as a function
        let execute_type: String = lua.load("return type(os.execute)").eval().unwrap();
        assert_eq!(execute_type, "function");

        // But os.time should NOT exist (not in custom spec)
        let result = lua.load("return os.time").eval::<mlua::Value>();
        match result {
            Ok(mlua::Value::Nil) => { /* expected */ }
            Err(_) => { /* also acceptable */ }
            Ok(_) => panic!("os.time should not exist in custom spec"),
        }
    }

    #[test]
    fn test_default_spec_includes_expected_functions() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        apply(&lua, policy, None).unwrap(); // Use default spec

        // Safe functions exist
        let time: i64 = lua.load("return os.time()").eval().unwrap();
        assert!(time > 0);

        let date: String = lua.load("return os.date('%Y')").eval().unwrap();
        assert_eq!(date.len(), 4);

        // Unsafe functions exist and are wrapped
        let execute_type: String = lua.load("return type(os.execute)").eval().unwrap();
        assert_eq!(execute_type, "function");

        let load_type: String = lua.load("return type(load)").eval().unwrap();
        assert_eq!(load_type, "function");
    }
}
