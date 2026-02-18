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

use std::sync::mpsc;

/// Intercepts `print()` calls and returns a channel receiver for captured output.
///
/// Replaces the global `print()` function with a version that sends output to a channel
/// instead of stdout. Arguments are converted to strings, joined with tabs, and have a
/// newline appended.
///
/// # Example
///
/// ```
/// use onetool::runtime::output;
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
/// let rx = output::capture_output(&lua)?;
///
/// lua.load(r#"print("hello", "world")"#).exec()?;
///
/// let output: Vec<String> = rx.try_iter().collect();
/// assert_eq!(output, vec!["hello\tworld\n"]);
/// # Ok(())
/// # }
/// ```
pub fn capture_output(lua: &mlua::Lua) -> mlua::Result<mpsc::Receiver<String>> {
    let (tx, rx) = mpsc::channel::<String>();
    let lua_tostring: mlua::Function = lua.globals().get("tostring")?;

    let print_fn = lua.create_function(move |_lua, args: mlua::MultiValue| {
        let mut line = args
            .iter()
            .map(|v| lua_tostring.call::<String>(v))
            .collect::<Result<Vec<_>, _>>()?
            .join("\t");
        line.push('\n');

        tx.send(line).expect("Failed to send output");

        Ok(())
    })?;

    lua.globals().set("print", print_fn)?;

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to collect all messages from a receiver
    fn collect_output(rx: mpsc::Receiver<String>) -> String {
        rx.try_iter().collect()
    }

    #[test]
    fn test_capture_output_single_print() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print("hello")"#).exec().unwrap();

        assert_eq!(collect_output(rx), "hello\n");
    }

    #[test]
    fn test_capture_output_multiple_prints() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(
            r#"
            print("line1")
            print("line2")
            print("line3")
        "#,
        )
        .exec()
        .unwrap();

        assert_eq!(collect_output(rx), "line1\nline2\nline3\n");
    }

    #[test]
    fn test_capture_output_returns_accumulated() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print("first")"#).exec().unwrap();
        lua.load(r#"print("second")"#).exec().unwrap();

        assert_eq!(collect_output(rx), "first\nsecond\n");
    }

    #[test]
    fn test_capture_output_with_multiple_args() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print("a", "b", "c")"#).exec().unwrap();

        assert_eq!(collect_output(rx), "a\tb\tc\n");
    }

    #[test]
    fn test_capture_output_with_newlines() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(
            r#"
            print("x")
            print("y")
        "#,
        )
        .exec()
        .unwrap();

        assert_eq!(collect_output(rx), "x\ny\n");
    }

    #[test]
    fn test_capture_output_converts_numbers() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print(42)"#).exec().unwrap();

        assert_eq!(collect_output(rx), "42\n");
    }

    #[test]
    fn test_capture_output_handles_nil() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print(nil)"#).exec().unwrap();

        assert_eq!(collect_output(rx), "nil\n");
    }

    #[test]
    fn test_capture_output_multiple_captures_on_same_lua() {
        let lua = mlua::Lua::new();
        let rx1 = capture_output(&lua).unwrap();

        lua.load(r#"print("first")"#).exec().unwrap();
        assert_eq!(rx1.try_iter().collect::<String>(), "first\n");

        // Install second capture - it replaces the print function
        let rx2 = capture_output(&lua).unwrap();

        lua.load(r#"print("second")"#).exec().unwrap();

        // First receiver no longer receives new messages (print was replaced)
        assert_eq!(rx1.try_iter().collect::<String>(), "");
        // Second receiver has new output
        assert_eq!(rx2.try_iter().collect::<String>(), "second\n");
    }

    #[test]
    fn test_capture_output_accumulates_across_evals() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print("first")"#).exec().unwrap();
        lua.load(r#"print("second")"#).exec().unwrap();
        lua.load(r#"print("third")"#).exec().unwrap();

        assert_eq!(collect_output(rx), "first\nsecond\nthird\n");
    }

    #[test]
    fn test_capture_output_empty_output() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        // Execute code that doesn't print
        lua.load(r#"local x = 42"#).exec().unwrap();

        assert_eq!(collect_output(rx), "");
    }

    #[test]
    fn test_capture_output_no_args() {
        let lua = mlua::Lua::new();
        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print()"#).exec().unwrap();

        assert_eq!(collect_output(rx), "\n");
    }

    #[test]
    fn test_capture_output_handles_tables() {
        let lua = mlua::Lua::new();

        let rx = capture_output(&lua).unwrap();

        lua.load(r#"print({x = 1})"#).exec().unwrap();
        let output = collect_output(rx);
        assert!(output.starts_with("table: 0x"));
    }
}
