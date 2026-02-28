use crate::runtime::{self, output, sandbox};
use std::sync::{Arc, Mutex};

/// Errors that can occur during REPL operations.
#[derive(Debug)]
pub enum ReplError {
    /// Error from the Lua runtime
    Lua(mlua::Error),
    /// The runtime lock was poisoned (panic in another thread while holding lock)
    LockPoisoned,
}

impl std::fmt::Display for ReplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplError::Lua(e) => write!(f, "Lua error: {}", e),
            ReplError::LockPoisoned => write!(f, "Runtime lock poisoned"),
        }
    }
}

impl std::error::Error for ReplError {}

impl From<mlua::Error> for ReplError {
    fn from(err: mlua::Error) -> Self {
        ReplError::Lua(err)
    }
}

/// Main interface for evaluating Lua code in a sandboxed environment.
///
/// `Repl` manages a Lua runtime and captures output from `print()` calls separately
/// from expression return values. State (variables, functions) persists between
/// evaluations.
///
/// # Thread Safety
///
/// `Repl` is `Send + Sync` and can be safely shared across threads. The Lua runtime
/// is wrapped in a `Mutex` to provide interior mutability and thread-safe concurrent
/// access. This is enabled by mlua's `send` feature flag, which makes the underlying
/// Lua VM thread-safe.
///
/// You can safely call methods on the same `Repl` from multiple threads. The mutex
/// ensures that evaluations are serialized and state remains consistent.
///
/// # Example
///
/// ```
/// use onetool::Repl;
///
/// # fn example() -> Result<(), mlua::Error> {
/// let repl = Repl::new()?;
///
/// // State persists across evaluations
/// repl.eval("x = 42")?;
/// let outcome = repl.eval("return x * 2")?;
///
/// assert_eq!(outcome.result.unwrap()[0], "84");
/// # Ok(())
/// # }
/// ```
pub struct Repl {
    runtime: Mutex<mlua::Lua>,
}

/// Result of evaluating Lua code.
///
/// Contains both the return values (or error) from the evaluation and any output
/// captured from `print()` calls during execution.
pub struct EvalOutcome {
    /// Evaluation result: `Ok(values)` for successful execution with formatted return
    /// values, or `Err(message)` for runtime/syntax/callback errors.
    pub result: Result<Vec<String>, String>,
    /// Lines captured from `print()` calls during execution. Each element includes
    /// a trailing newline.
    pub output: Vec<String>,
}

impl Repl {
    /// Creates a new sandboxed Lua REPL.
    ///
    /// The runtime has dangerous operations blocked (file I/O, code loading, OS commands)
    /// while preserving safe Lua standard library functions.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use onetool::Repl;
    ///
    /// # fn example() -> Result<(), mlua::Error> {
    /// let repl = Repl::new()?;
    /// let outcome = repl.eval("return math.sqrt(16)")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Result<Self, mlua::Error> {
        Self::new_with(runtime::default()?)
    }

    /// Creates a REPL with a custom Lua runtime.
    ///
    /// Useful when you need to register custom functions or globals.
    /// Note that sandboxing is NOT automatically applied to the provided runtime.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use onetool::{Repl, runtime};
    ///
    /// # fn example() -> Result<(), mlua::Error> {
    /// let lua = mlua::Lua::new();
    ///
    /// // Apply sandboxing FIRST (it clears globals)
    /// runtime::sandbox::apply(&lua)?;
    ///
    /// // Register custom functions AFTER sandboxing
    /// lua.globals().set("custom_value", 42)?;
    ///
    /// let repl = Repl::new_with(lua)?;
    /// let outcome = repl.eval("return custom_value")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security Warning
    ///
    /// When using `new_with`, you are responsible for sandboxing the Lua runtime BEFORE
    /// registering custom functions. The sandbox uses `globals.clear()` which will destroy
    /// any globals registered before sandboxing is applied.
    ///
    /// **Recommended pattern**:
    /// 1. Create Lua runtime
    /// 2. Apply sandboxing with `runtime::sandbox::apply()`
    /// 3. Register custom functions AFTER sandboxing
    /// 4. Create REPL with `new_with()`
    ///
    /// **Not recommended**:
    /// 1. Create Lua runtime
    /// 2. Register custom functions ← These will be destroyed by sandboxing!
    /// 3. Apply sandboxing
    /// 4. Create REPL
    pub fn new_with(runtime: mlua::Lua) -> Result<Self, mlua::Error> {
        let runtime = Mutex::new(runtime);

        Ok(Self { runtime })
    }

    /// Creates a sandboxed REPL with a custom access control policy.
    ///
    /// This constructor automatically creates a fresh Lua runtime and applies policy-based
    /// sandboxing, giving you fine-grained control over which unsafe operations are allowed.
    ///
    /// # When to Use Each Constructor
    ///
    /// - [`new()`](Repl::new) - Default sandboxing (blocks all unsafe operations)
    /// - **`new_with_policy()`** - Custom policy for selective unsafe operation control
    /// - [`new_with()`](Repl::new_with) - Manual sandboxing and custom functions
    ///
    /// Use this constructor when you need to allow specific unsafe operations while blocking
    /// others. For example, you might allow filesystem reads but block writes, or allow
    /// specific system commands while denying others.
    ///
    /// # Parameters
    ///
    /// * `policy` - An [`Arc`]-wrapped policy implementing [`Policy`](sandbox::policy::Policy).
    ///              The policy's `check_access()` method is called for each unsafe operation.
    ///
    /// # Example: Custom Policy
    ///
    /// ```
    /// use onetool::{Repl, runtime::sandbox::policy::{Policy, Action, Decision}};
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Result<(), mlua::Error> {
    /// // Policy that only allows string.upper but blocks other unsafe operations
    /// struct SelectivePolicy;
    ///
    /// impl Policy for SelectivePolicy {
    ///     fn check_access(&self, action: &Action) -> Decision {
    ///         match action {
    ///             Action::CallFunction { name, .. } if name == "string.upper" => {
    ///                 Decision::Allow
    ///             }
    ///             _ => Decision::Deny("Not in allowlist".to_string())
    ///         }
    ///     }
    /// }
    ///
    /// let repl = Repl::new_with_policy(Arc::new(SelectivePolicy))?;
    ///
    /// // Allowed operation succeeds
    /// let outcome = repl.eval(r#"return string.upper("hello")"#)?;
    /// assert!(outcome.result.unwrap()[0].contains("HELLO"));
    ///
    /// // Blocked operation returns nil
    /// let outcome = repl.eval(r#"return io.open("file.txt")"#)?;
    /// assert!(outcome.result.unwrap()[0].to_lowercase().contains("nil"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Combining with Custom Functions
    ///
    /// You can use [`with_runtime()`](Repl::with_runtime) to add custom Rust functions
    /// after creating a policy-based REPL:
    ///
    /// ```
    /// use onetool::{Repl, runtime::sandbox::policy::DenyAllPolicy};
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new_with_policy(Arc::new(DenyAllPolicy))?;
    ///
    /// // Add custom functions after sandboxing
    /// repl.with_runtime(|lua| {
    ///     let greet = lua.create_function(|_, name: String| {
    ///         Ok(format!("Hello, {}!", name))
    ///     })?;
    ///     lua.globals().set("greet", greet)?;
    ///     Ok(())
    /// })?;
    ///
    /// let outcome = repl.eval(r#"return greet("World")"#)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # See Also
    ///
    /// - [`Policy`](sandbox::policy::Policy) trait - Implement custom access control
    /// - [`DenyAllPolicy`](sandbox::policy::DenyAllPolicy) - Built-in restrictive policy
    /// - [`DangerousAllowAllPolicy`](sandbox::policy::DangerousAllowAllPolicy) - Built-in permissive policy (use with caution)
    /// - [`runtime::sandbox::apply_with_policy()`] - Low-level policy application API
    /// - [`with_runtime()`](Repl::with_runtime) - Add custom functions post-sandboxing
    pub fn new_with_policy<P: sandbox::policy::Policy + 'static>(
        policy: Arc<P>,
    ) -> Result<Self, mlua::Error> {
        let runtime = mlua::Lua::new();
        sandbox::apply_with_policy(&runtime, policy, None)?;
        Self::new_with(runtime)
    }

    /// Evaluates Lua code and captures output.
    ///
    /// Returns both the expression return values and any output from `print()` calls.
    /// State persists between calls, so variables and functions remain available.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use onetool::Repl;
    ///
    /// # fn example() -> Result<(), mlua::Error> {
    /// let repl = Repl::new()?;
    ///
    /// let outcome = repl.eval(r#"
    ///     print("debug message")
    ///     return 1, 2, 3
    /// "#)?;
    ///
    /// assert_eq!(outcome.output, vec!["debug message\n"]);
    /// assert_eq!(outcome.result.unwrap(), vec!["1", "2", "3"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn eval(&self, code: &str) -> Result<EvalOutcome, mlua::Error> {
        let runtime = self.runtime.lock().unwrap();

        let (eval_result, output) = output::with_output_capture(&runtime, |runtime| {
            runtime.load(code).eval::<mlua::MultiValue>()
        })?;

        let result = match eval_result {
            Ok(values) => Ok(values
                .iter()
                .map(|v| format!("{:#?}", v))
                .collect::<Vec<_>>()),
            Err(e) => Err(Self::format_lua_error(&e)),
        };

        Ok(EvalOutcome { result, output })
    }

    /// Provides temporary access to the underlying Lua runtime.
    ///
    /// This method allows you to perform advanced operations that aren't exposed
    /// by the REPL's main API, such as registering custom Rust functions or
    /// modifying global state.
    ///
    /// # Note on Thread Safety
    ///
    /// The Lua VM is thread-safe (enabled by mlua's `send` feature). The Mutex
    /// provides interior mutability and ensures thread-safe concurrent access.
    /// Multiple threads can safely call this method on the same `Repl` instance.
    ///
    /// # Parameters
    ///
    /// * `f` - A closure that receives an immutable reference to the Lua runtime
    ///         and can return any type wrapped in `Result<T, mlua::Error>`
    ///
    /// # Returns
    ///
    /// Returns whatever the closure returns, or a `ReplError` if the lock is poisoned
    /// or the Lua operation fails.
    ///
    /// # Examples
    ///
    /// ## Registering a Custom Rust Function
    ///
    /// ```
    /// use onetool::Repl;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new()?;
    ///
    /// // Register a function with captured state
    /// let multiplier = 10;
    /// repl.with_runtime(|lua| {
    ///     let func = lua.create_function(move |_, x: i32| {
    ///         Ok(x * multiplier)
    ///     })?;
    ///     lua.globals().set("multiply", func)?;
    ///     Ok(())
    /// })?;
    ///
    /// let result = repl.eval("return multiply(5)")?;
    /// assert_eq!(result.result.unwrap()[0], "50");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Setting Global Variables
    ///
    /// ```
    /// use onetool::Repl;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new()?;
    ///
    /// // Set a global variable from Rust
    /// repl.with_runtime(|lua| {
    ///     lua.globals().set("API_KEY", "secret-key-123")?;
    ///     lua.globals().set("MAX_RETRIES", 3)?;
    ///     Ok(())
    /// })?;
    ///
    /// let result = repl.eval("return API_KEY, MAX_RETRIES")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Extracting Values from Lua
    ///
    /// ```
    /// use onetool::Repl;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new()?;
    /// repl.eval("counter = 42")?;
    ///
    /// // Extract a value from Lua to Rust
    /// let counter: i32 = repl.with_runtime(|lua| {
    ///     lua.globals().get("counter")
    /// })?;
    ///
    /// assert_eq!(counter, 42);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Registering Custom Modules
    ///
    /// ```
    /// use onetool::{Repl, runtime::docs};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let repl = Repl::new()?;
    ///
    /// repl.with_runtime(|lua| {
    ///     // Create a module table
    ///     let utils = lua.create_table()?;
    ///
    ///     // Add functions to the module
    ///     let double = lua.create_function(|_, x: i32| Ok(x * 2))?;
    ///     utils.set("double", double)?;
    ///
    ///     // Register the module
    ///     lua.globals().set("utils", utils)?;
    ///
    ///     // Register documentation
    ///     docs::register(lua, &docs::LuaDoc {
    ///         name: "utils".to_string(),
    ///         typ: docs::LuaDocTyp::Scope,
    ///         description: "Utility functions".to_string(),
    ///     })?;
    ///
    ///     Ok(())
    /// })?;
    ///
    /// let result = repl.eval("return utils.double(5)")?;
    /// assert_eq!(result.result.unwrap()[0], "10");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## See Also
    ///
    /// - [`new_with()`](Repl::new_with) for pre-sandboxing extension
    /// - [`runtime::docs::register()`] for making functions discoverable
    /// - `examples/custom-functions.rs` for complete patterns
    pub fn with_runtime<F, R>(&self, f: F) -> Result<R, ReplError>
    where
        F: FnOnce(&mlua::Lua) -> Result<R, mlua::Error>,
    {
        let runtime = self.runtime.lock().map_err(|_| ReplError::LockPoisoned)?;
        f(&runtime).map_err(ReplError::from)
    }

    fn format_lua_error(error: &mlua::Error) -> String {
        match error {
            mlua::Error::RuntimeError(msg) => format!("RuntimeError: {}", msg),
            mlua::Error::SyntaxError { message, .. } => format!("SyntaxError: {}", message),
            mlua::Error::MemoryError(msg) => format!("MemoryError: {}", msg),
            mlua::Error::CallbackError { traceback, cause } => {
                format!("CallbackError: {}\nTraceback:\n{}", cause, traceback)
            }
            _ => format!("{}", error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a new REPL instance
    fn create_repl() -> Repl {
        Repl::new().expect("Failed to create REPL")
    }

    // Helper function to assert that a result contains an error with expected substring
    fn assert_error_contains(result: &Result<Vec<String>, String>, expected: &str) {
        assert!(result.is_err(), "Expected error but got success");
        let error = result.as_ref().unwrap_err();
        assert!(
            error.contains(expected),
            "Expected error to contain '{}', but got: {}",
            expected,
            error
        );
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn repl_is_send_sync() {
        assert_send_sync::<Repl>();
    }

    // === A. Initialization Tests ===

    #[test]
    fn test_new_creates_repl_successfully() {
        let result = Repl::new();
        assert!(result.is_ok(), "Failed to create REPL: {:?}", result.err());
    }

    #[test]
    fn test_new_with_custom_runtime() {
        let lua = mlua::Lua::new();
        // Set a global variable
        lua.globals().set("test_var", 42).unwrap();

        let repl = Repl::new_with(lua).unwrap();
        let eval = repl.eval("return test_var").unwrap();

        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "42");
    }

    #[test]
    fn test_new_applies_sandboxing() {
        let repl = create_repl();
        let eval = repl.eval("return io.open('test.txt', 'r')").unwrap();

        // With policy-based sandbox, io.open returns nil (denied by DenyAllPolicy)
        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result[0].to_lowercase().contains("nil"));
    }

    // === B. Successful Evaluation Tests ===

    #[test]
    fn test_eval_simple_expression() {
        let repl = create_repl();
        let eval = repl.eval("1 + 1").unwrap();

        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "2");
        assert!(eval.output.is_empty());
    }

    #[test]
    fn test_eval_string_expression() {
        let repl = create_repl();
        let eval = repl.eval(r#"return "hello""#).unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("hello"));
    }

    #[test]
    fn test_eval_multiple_return_values() {
        let repl = create_repl();
        let eval = repl.eval("return 1, 2, 3").unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "1");
        assert_eq!(result[1], "2");
        assert_eq!(result[2], "3");
    }

    #[test]
    fn test_eval_nil_value() {
        let repl = create_repl();
        let eval = repl.eval("return nil").unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert_eq!(result.len(), 1);
        // Debug format uses "Nil" in mlua
        assert!(result[0].to_lowercase().contains("nil"));
    }

    #[test]
    fn test_eval_boolean_values() {
        let repl = create_repl();
        let eval_true = repl.eval("return true").unwrap();
        let eval_false = repl.eval("return false").unwrap();

        assert!(eval_true.result.is_ok());
        let result_true = eval_true.result.unwrap();
        assert!(result_true[0].contains("true"));

        assert!(eval_false.result.is_ok());
        let result_false = eval_false.result.unwrap();
        assert!(result_false[0].contains("false"));
    }

    #[test]
    fn test_eval_table_expression() {
        let repl = create_repl();
        let eval = repl.eval("return {x=1, y=2}").unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        // Tables have a representation like "Table(...)" or similar
        assert!(!result.is_empty());
    }

    #[test]
    fn test_eval_function_return() {
        let repl = create_repl();
        let eval = repl.eval(r#"return string.upper("hello")"#).unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result[0].contains("HELLO"));
    }

    #[test]
    fn test_eval_empty_code() {
        let repl = create_repl();
        let eval = repl.eval("").unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result.is_empty());
        assert!(eval.output.is_empty());
    }

    #[test]
    fn test_eval_assignment_no_return() {
        let repl = create_repl();
        let eval = repl.eval("x = 42").unwrap();

        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result.is_empty());
    }

    // === C. Output Capture Tests ===

    #[test]
    fn test_eval_captures_print_output() {
        let repl = create_repl();
        let eval = repl.eval(r#"print("test")"#).unwrap();

        assert_eq!(eval.output, vec!["test\n"]);
        assert!(eval.result.is_ok());
        assert!(eval.result.unwrap().is_empty());
    }

    #[test]
    fn test_eval_captures_multiple_prints() {
        let repl = create_repl();
        let eval = repl
            .eval(
                r#"
            print("line1")
            print("line2")
            print("line3")
        "#,
            )
            .unwrap();

        assert_eq!(eval.output, vec!["line1\n", "line2\n", "line3\n"]);
    }

    #[test]
    fn test_eval_captures_print_with_multiple_args() {
        let repl = create_repl();
        let eval = repl.eval(r#"print("a", "b", "c")"#).unwrap();

        assert_eq!(eval.output, vec!["a\tb\tc\n"]);
    }

    #[test]
    fn test_eval_print_and_return_separate() {
        let repl = create_repl();
        let eval = repl
            .eval(
                r#"
            print("output")
            return 42
        "#,
            )
            .unwrap();

        assert_eq!(eval.output, vec!["output\n"]);
        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "42");
    }

    #[test]
    fn test_eval_print_various_types() {
        let repl = create_repl();
        let eval = repl.eval(r#"print(42, nil, true, false)"#).unwrap();

        assert_eq!(eval.output, vec!["42\tnil\ttrue\tfalse\n"]);
    }

    #[test]
    fn test_eval_output_not_accumulated() {
        let repl = create_repl();

        let eval1 = repl.eval(r#"print("first")"#).unwrap();
        assert_eq!(eval1.output, vec!["first\n"]);

        let eval2 = repl.eval(r#"print("second")"#).unwrap();
        assert_eq!(eval2.output, vec!["second\n"]);
    }

    // === D. Error Handling Tests ===

    #[test]
    fn test_eval_syntax_error() {
        let repl = create_repl();
        let eval = repl.eval("function end").unwrap();

        assert_error_contains(&eval.result, "SyntaxError:");
    }

    #[test]
    fn test_eval_runtime_error() {
        let repl = create_repl();
        let eval = repl.eval(r#"error("test error")"#).unwrap();

        assert_error_contains(&eval.result, "RuntimeError:");
        assert_error_contains(&eval.result, "test error");
    }

    #[test]
    fn test_eval_undefined_variable_error() {
        let repl = create_repl();
        // In Lua, accessing undefined variables returns nil, not an error.
        // To get an error, we need to call a nil value or access a field on nil.
        let eval = repl.eval("undefined_var()").unwrap();

        assert_error_contains(&eval.result, "RuntimeError:");
    }

    #[test]
    fn test_eval_type_error() {
        let repl = create_repl();
        let eval = repl.eval(r#"return "string" + 1"#).unwrap();

        assert!(eval.result.is_err());
    }

    #[test]
    fn test_eval_callback_error() {
        let lua = mlua::Lua::new();

        // Create Rust function that errors
        let error_fn = lua
            .create_function(|_, ()| -> mlua::Result<()> {
                Err(mlua::Error::RuntimeError("callback failed".to_string()))
            })
            .unwrap();
        lua.globals().set("error_fn", error_fn).unwrap();

        let repl = Repl::new_with(lua).unwrap();
        let eval = repl.eval("error_fn()").unwrap();

        assert_error_contains(&eval.result, "CallbackError:");
        assert_error_contains(&eval.result, "callback failed");
    }

    #[test]
    fn test_eval_blocked_function_error() {
        let repl = create_repl();
        let eval = repl.eval(r#"return io.open("file.txt")"#).unwrap();

        // Now returns nil instead of erroring
        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result[0].to_lowercase().contains("nil"));
    }

    #[test]
    fn test_eval_error_preserves_output() {
        let repl = create_repl();
        let eval = repl
            .eval(
                r#"
            print("before error")
            error("test error")
        "#,
            )
            .unwrap();

        assert_eq!(eval.output, vec!["before error\n"]);
        assert_error_contains(&eval.result, "RuntimeError:");
    }

    // === E. State Persistence Tests ===

    #[test]
    fn test_eval_state_persists_between_calls() {
        let repl = create_repl();

        let eval1 = repl.eval("x = 42").unwrap();
        assert!(eval1.result.is_ok());

        let eval2 = repl.eval("return x").unwrap();
        assert!(eval2.result.is_ok());
        assert_eq!(eval2.result.unwrap()[0], "42");
    }

    #[test]
    fn test_eval_function_definition_persists() {
        let repl = create_repl();

        let eval1 = repl.eval("function double(n) return n * 2 end").unwrap();
        assert!(eval1.result.is_ok());

        let eval2 = repl.eval("return double(21)").unwrap();
        assert!(eval2.result.is_ok());
        assert_eq!(eval2.result.unwrap()[0], "42");
    }

    #[test]
    fn test_eval_global_table_persists() {
        let repl = create_repl();

        let eval1 = repl.eval("my_table = {x = 10}").unwrap();
        assert!(eval1.result.is_ok());

        let eval2 = repl.eval("return my_table.x").unwrap();
        assert!(eval2.result.is_ok());
        assert_eq!(eval2.result.unwrap()[0], "10");
    }

    #[test]
    fn test_eval_table_modification_persists() {
        let repl = create_repl();

        repl.eval("my_table = {x = 10}").unwrap();
        repl.eval("my_table.x = 20").unwrap();

        let eval = repl.eval("return my_table.x").unwrap();
        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "20");
    }

    // === F. Integration Tests ===

    #[test]
    fn test_integration_with_safe_os_functions() {
        let repl = create_repl();

        // os.time should work
        let eval = repl.eval("return os.time()").unwrap();
        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(!result.is_empty());
        // Should be a number (timestamp)
        assert!(result[0].parse::<i64>().is_ok());
    }

    #[test]
    fn test_integration_math_functions() {
        let repl = create_repl();

        let eval = repl.eval("return math.sqrt(16)").unwrap();
        assert!(eval.result.is_ok());
        // Lua may format as "4" or "4.0" depending on the value
        let result = eval.result.unwrap()[0].clone();
        assert!(result == "4" || result == "4.0");
    }

    #[test]
    fn test_integration_string_functions() {
        let repl = create_repl();

        let eval = repl.eval(r#"return string.upper("test")"#).unwrap();
        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result[0].contains("TEST"));
    }

    #[test]
    fn test_integration_table_functions() {
        let repl = create_repl();

        let eval = repl
            .eval(r#"return table.concat({"a", "b", "c"}, ",")"#)
            .unwrap();
        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result[0].contains("a,b,c"));
    }

    // === G. Runtime Access Tests ===

    #[test]
    fn test_with_runtime_set_global_variable() {
        let repl = create_repl();

        let result = repl.with_runtime(|lua| {
            lua.globals().set("custom_var", 42)?;
            Ok(())
        });

        assert!(result.is_ok());

        let eval = repl.eval("return custom_var").unwrap();
        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "42");
    }

    #[test]
    fn test_with_runtime_register_rust_function() {
        let repl = create_repl();

        let result = repl.with_runtime(|lua| {
            let greet = lua.create_function(|_, name: String| Ok(format!("Hello, {}!", name)))?;
            lua.globals().set("greet", greet)?;
            Ok(())
        });

        assert!(result.is_ok());

        let eval = repl.eval(r#"return greet("World")"#).unwrap();
        assert!(eval.result.is_ok());
        let result = eval.result.unwrap();
        assert!(result[0].contains("Hello, World!"));
    }

    #[test]
    fn test_with_runtime_closure_captures_state() {
        let repl = create_repl();

        let multiplier = 10;
        let result = repl.with_runtime(|lua| {
            let func = lua.create_function(move |_, x: i32| Ok(x * multiplier))?;
            lua.globals().set("multiply", func)?;
            Ok(())
        });

        assert!(result.is_ok());

        let eval = repl.eval("return multiply(5)").unwrap();
        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "50");
    }

    #[test]
    fn test_with_runtime_extract_value_from_lua() {
        let repl = create_repl();
        repl.eval("x = 42").unwrap();

        let value: i32 = repl.with_runtime(|lua| lua.globals().get("x")).unwrap();

        assert_eq!(value, 42);
    }

    #[test]
    fn test_with_runtime_extract_string_from_lua() {
        let repl = create_repl();
        repl.eval(r#"name = "Alice""#).unwrap();

        let value: String = repl.with_runtime(|lua| lua.globals().get("name")).unwrap();

        assert_eq!(value, "Alice");
    }

    #[test]
    fn test_with_runtime_returns_custom_type() {
        let repl = create_repl();
        repl.eval("a = 10; b = 20").unwrap();

        let sum: i32 = repl
            .with_runtime(|lua| {
                let a: i32 = lua.globals().get("a")?;
                let b: i32 = lua.globals().get("b")?;
                Ok(a + b)
            })
            .unwrap();

        assert_eq!(sum, 30);
    }

    #[test]
    fn test_with_runtime_error_propagation() {
        let repl = create_repl();

        let result: Result<(), ReplError> = repl.with_runtime(|lua| {
            // Try to get a non-existent global as a number (will fail)
            let _val: i32 = lua.globals().get("nonexistent")?;
            Ok(())
        });

        assert!(result.is_err());
        match result {
            Err(ReplError::Lua(_)) => {}
            _ => panic!("Expected Lua error"),
        }
    }

    #[test]
    fn test_with_runtime_multiple_operations() {
        let repl = create_repl();

        repl.with_runtime(|lua| {
            lua.globals().set("a", 1)?;
            lua.globals().set("b", 2)?;
            lua.globals().set("c", 3)?;
            Ok(())
        })
        .unwrap();

        let eval = repl.eval("return a + b + c").unwrap();
        assert!(eval.result.is_ok());
        assert_eq!(eval.result.unwrap()[0], "6");
    }

    #[test]
    fn test_with_runtime_state_persists_after_call() {
        let repl = create_repl();

        // First call sets a global
        repl.with_runtime(|lua| {
            lua.globals().set("persistent", 99)?;
            Ok(())
        })
        .unwrap();

        // Second call should see the same global
        let value: i32 = repl
            .with_runtime(|lua| lua.globals().get("persistent"))
            .unwrap();

        assert_eq!(value, 99);
    }
}
