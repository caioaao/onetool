use crate::runtime;
use std::sync::{Mutex, mpsc};

/// Main interface for evaluating Lua code in a sandboxed environment.
///
/// `Repl` manages a Lua runtime and captures output from `print()` calls separately
/// from expression return values. State (variables, functions) persists between
/// evaluations.
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
    // TODO: check if mutex is actually necessary
    runtime: Mutex<mlua::Lua>,
    output_receiver: mpsc::Receiver<String>,
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
    /// Useful when you need to register custom globals or functions before sandboxing.
    /// Note that sandboxing is NOT automatically applied to the provided runtime.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use onetool::{Repl, runtime};
    ///
    /// # fn example() -> Result<(), mlua::Error> {
    /// let lua = mlua::Lua::new();
    /// lua.globals().set("custom_value", 42)?;
    /// runtime::sandbox::apply(&lua)?;  // Apply sandboxing manually
    ///
    /// let repl = Repl::new_with(lua)?;
    /// let outcome = repl.eval("return custom_value")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_with(runtime: mlua::Lua) -> Result<Self, mlua::Error> {
        let output_receiver = runtime::output::capture_output(&runtime)?;
        let runtime = Mutex::new(runtime);

        Ok(Self {
            runtime,
            output_receiver,
        })
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

        let result = match runtime.load(code).eval::<mlua::MultiValue>() {
            Ok(values) => Ok(values
                .iter()
                .map(|v| format!("{:#?}", v))
                .collect::<Vec<_>>()),
            Err(e) => Err(Self::format_lua_error(&e)),
        };

        let output = self.output_receiver.try_iter().collect();

        Ok(EvalOutcome { result, output })
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
        let eval = repl.eval("io.open('test.txt', 'r')").unwrap();

        assert_error_contains(&eval.result, "nil");
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
        let eval = repl.eval(r#"io.open("file.txt")"#).unwrap();

        assert_error_contains(&eval.result, "nil");
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
}
