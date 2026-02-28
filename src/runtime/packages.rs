//! Lua package path management.
//!
//! Provides utilities for extending the Lua `package.path` so that `require()` can
//! find external modules. Note that `require()` is blocked by the default sandbox
//! policy — you must use a custom policy (or [`DangerousAllowAllPolicy`](super::sandbox::policy::DangerousAllowAllPolicy))
//! to allow package loading.

/// Appends additional search patterns to Lua's `package.path`.
///
/// Each entry in `extra` should follow Lua's path pattern convention, using `?` as
/// a placeholder for the module name. For example, `"./lib/?.lua"` allows
/// `require("foo")` to find `./lib/foo.lua`.
///
/// Entries are appended after the existing path, preserving the original search order.
pub fn extend_path(runtime: &mlua::Lua, extra: &[&str]) -> mlua::Result<()> {
    let package: mlua::Table = runtime.globals().get("package")?;

    let base: String = package.get("path")?;

    let mut parts = vec![base];
    parts.extend(extra.iter().map(|s| s.to_string()));

    package.set("path", parts.join(";"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Helper to get the current package.path value
    fn get_package_path(lua: &mlua::Lua) -> mlua::Result<String> {
        let package: mlua::Table = lua.globals().get("package")?;
        package.get("path")
    }

    #[test]
    fn test_extend_path_single_entry() {
        let lua = mlua::Lua::new();
        let original_path = get_package_path(&lua).unwrap();

        extend_path(&lua, &["./custom/?.lua"]).unwrap();

        let new_path = get_package_path(&lua).unwrap();
        assert!(new_path.starts_with(&original_path));
        assert!(new_path.contains("./custom/?.lua"));
        assert_eq!(new_path, format!("{};./custom/?.lua", original_path));
    }

    #[test]
    fn test_extend_path_multiple_entries() {
        let lua = mlua::Lua::new();
        let original_path = get_package_path(&lua).unwrap();

        extend_path(&lua, &["./lib1/?.lua", "./lib2/?.lua", "./lib3/?.lua"]).unwrap();

        let new_path = get_package_path(&lua).unwrap();
        assert!(new_path.starts_with(&original_path));
        assert!(new_path.contains("./lib1/?.lua"));
        assert!(new_path.contains("./lib2/?.lua"));
        assert!(new_path.contains("./lib3/?.lua"));

        // Verify order is preserved
        let expected = format!("{};./lib1/?.lua;./lib2/?.lua;./lib3/?.lua", original_path);
        assert_eq!(new_path, expected);
    }

    #[test]
    fn test_extend_path_preserves_original() {
        let lua = mlua::Lua::new();
        let original_path = get_package_path(&lua).unwrap();

        extend_path(&lua, &["./extra/?.lua"]).unwrap();

        let new_path = get_package_path(&lua).unwrap();
        // Original path should be at the beginning
        assert!(new_path.starts_with(&original_path));
        // And it should still be searchable (not replaced)
        let paths: Vec<&str> = new_path.split(';').collect();
        let original_paths: Vec<&str> = original_path.split(';').collect();
        for orig_path in original_paths {
            assert!(paths.contains(&orig_path));
        }
    }

    #[test]
    fn test_extend_path_empty_array() {
        let lua = mlua::Lua::new();
        let original_path = get_package_path(&lua).unwrap();

        extend_path(&lua, &[]).unwrap();

        let new_path = get_package_path(&lua).unwrap();
        // With empty array, path should remain unchanged
        assert_eq!(new_path, original_path);
    }

    #[test]
    fn test_load_module_from_extended_path() {
        let lua = mlua::Lua::new();

        // Create a temporary directory with a test module
        let temp_dir = tempfile::tempdir().unwrap();
        let module_path = temp_dir.path().join("testmod.lua");

        let mut file = fs::File::create(&module_path).unwrap();
        writeln!(
            file,
            r#"
local M = {{}}
M.greeting = "Hello from testmod!"
M.add = function(a, b) return a + b end
return M
"#
        )
        .unwrap();

        // Extend path to include our temp directory
        let search_pattern = format!("{}/?.lua", temp_dir.path().display());
        extend_path(&lua, &[&search_pattern]).unwrap();

        // Try to require the module
        let result = lua
            .load(
                r#"
local mod = require('testmod')
return mod.greeting, mod.add(2, 3)
"#,
            )
            .eval::<(String, i32)>();

        assert!(result.is_ok());
        let (greeting, sum) = result.unwrap();
        assert_eq!(greeting, "Hello from testmod!");
        assert_eq!(sum, 5);
    }

    #[test]
    fn test_paths_with_spaces() {
        let lua = mlua::Lua::new();
        let original_path = get_package_path(&lua).unwrap();

        // Test path with spaces
        extend_path(&lua, &["./my libs/?.lua"]).unwrap();

        let new_path = get_package_path(&lua).unwrap();
        assert!(new_path.contains("./my libs/?.lua"));
        assert_eq!(new_path, format!("{};./my libs/?.lua", original_path));
    }

    #[test]
    fn test_multiple_extensions_cumulative() {
        let lua = mlua::Lua::new();
        let original_path = get_package_path(&lua).unwrap();

        // First extension
        extend_path(&lua, &["./lib1/?.lua"]).unwrap();
        let path_after_first = get_package_path(&lua).unwrap();
        assert!(path_after_first.contains("./lib1/?.lua"));

        // Second extension should be cumulative
        extend_path(&lua, &["./lib2/?.lua"]).unwrap();
        let path_after_second = get_package_path(&lua).unwrap();

        // Both paths should be present
        assert!(path_after_second.contains("./lib1/?.lua"));
        assert!(path_after_second.contains("./lib2/?.lua"));

        // Original path should still be at the beginning
        assert!(path_after_second.starts_with(&original_path));
    }
}
