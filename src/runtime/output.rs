//! Output capture for Lua `print()` calls.
//!
//! This module intercepts Lua's global `print()` function to capture output separately
//! from expression return values. This allows distinguishing between debug output
//! (from `print()`) and actual evaluation results.
//!
//! # Output Format
//!
//! Each `print()` call produces one string with:
//! - Arguments separated by tabs
//! - A trailing newline
//!
//! For example, `print("a", "b", "c")` produces `"a\tb\tc\n"`.

/// Executes a closure with `print()` output capture, returning both the result and captured output.
///
/// Temporarily replaces the global `print()` function with a version that captures output.
/// Arguments are converted to strings, joined with tabs, and have a newline appended.
/// After the closure completes, the original `print()` function is restored.
///
/// # Returns
///
/// A tuple of:
/// - The result of the closure execution (which may be `Ok` or `Err`)
/// - A vector of captured output strings
///
/// # Output Format
///
/// Each `print()` call produces one string with:
/// - Arguments separated by tabs
/// - A trailing newline
///
/// For example, `print("a", "b", "c")` produces `"a\tb\tc\n"`.
///
/// # Example
///
/// ```
/// use onetool::runtime::output;
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let (result, output) = output::with_output_capture(&lua, |lua| {
///     lua.load(r#"print("hello", "world")"#).exec()?;
///     lua.load(r#"return 42"#).eval::<i32>()
/// })?;
///
/// assert_eq!(result.unwrap(), 42);
/// assert_eq!(output, vec!["hello\tworld\n"]);
/// # Ok(())
/// # }
/// ```
pub fn with_output_capture<F, R>(
    lua: &mlua::Lua,
    f: F,
) -> Result<(Result<R, mlua::Error>, Vec<String>), mlua::Error>
where
    F: FnOnce(&mlua::Lua) -> Result<R, mlua::Error>,
{
    let mut output_buf: Vec<String> = Vec::new();

    let result = lua.scope(|scope| {
        lua.globals().set(
            "print",
            scope.create_function_mut(|_, args: mlua::MultiValue| {
                let lua_tostring: mlua::Function = lua.globals().get("tostring")?;
                let mut line = args
                    .iter()
                    .map(|v| lua_tostring.call::<String>(v))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\t");
                line.push('\n');

                output_buf.push(line);

                Ok(())
            })?,
        )?;

        f(lua)
    });

    Ok((result, output_buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_output_capture_single_print() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print("hello")"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["hello\n"]);
    }

    #[test]
    fn test_with_output_capture_multiple_prints() {
        let lua = mlua::Lua::new();

        let (result, output) = with_output_capture(&lua, |lua| {
            lua.load(
                r#"
                print("line1")
                print("line2")
                print("line3")
            "#,
            )
            .exec()
        })
        .unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["line1\n", "line2\n", "line3\n"]);
    }

    #[test]
    fn test_with_output_capture_multiple_args() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print("a", "b", "c")"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["a\tb\tc\n"]);
    }

    #[test]
    fn test_with_output_capture_converts_numbers() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print(42)"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["42\n"]);
    }

    #[test]
    fn test_with_output_capture_handles_nil() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print(nil)"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["nil\n"]);
    }

    #[test]
    fn test_with_output_capture_handles_booleans() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print(true, false)"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["true\tfalse\n"]);
    }

    #[test]
    fn test_with_output_capture_empty_output() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"local x = 42"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, Vec::<String>::new());
    }

    #[test]
    fn test_with_output_capture_no_args() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print()"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output, vec!["\n"]);
    }

    #[test]
    fn test_with_output_capture_handles_tables() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"print({x = 1})"#).exec()).unwrap();

        assert!(result.is_ok());
        assert_eq!(output.len(), 1);
        assert!(output[0].starts_with("table: 0x"));
    }

    #[test]
    fn test_with_output_capture_returns_value() {
        let lua = mlua::Lua::new();

        let (result, output) = with_output_capture(&lua, |lua| {
            lua.load(r#"print("test"); return 42"#).eval::<i32>()
        })
        .unwrap();

        assert_eq!(result.unwrap(), 42);
        assert_eq!(output, vec!["test\n"]);
    }

    #[test]
    fn test_with_output_capture_captures_error() {
        let lua = mlua::Lua::new();

        let (result, output) =
            with_output_capture(&lua, |lua| lua.load(r#"error("test error")"#).exec()).unwrap();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test error"));
        assert_eq!(output, Vec::<String>::new());
    }

    #[test]
    fn test_with_output_capture_output_before_error() {
        let lua = mlua::Lua::new();

        let (result, output) = with_output_capture(&lua, |lua| {
            lua.load(r#"print("before"); error("test error")"#).exec()
        })
        .unwrap();

        assert!(result.is_err());
        assert_eq!(output, vec!["before\n"]);
    }
}
