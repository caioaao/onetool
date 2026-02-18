//! Runtime documentation system.
//!
//! This module provides a way to register documentation metadata about Lua functions
//! and scopes into a global `docs` table that Lua code can query at runtime.
//!
//! Documentation entries are formatted as `"(<type>) <description>"` and stored in
//! the `docs` global table indexed by name.
//!
//! # Example
//!
//! ```
//! use onetool::runtime::docs::{register, LuaDoc, LuaDocTyp};
//!
//! # fn example() -> mlua::Result<()> {
//! let lua = mlua::Lua::new();
//!
//! register(&lua, &LuaDoc {
//!     name: "my_fn".to_string(),
//!     typ: LuaDocTyp::Function,
//!     description: "Does something useful".to_string(),
//! })?;
//!
//! // Now accessible from Lua
//! let doc: String = lua.load(r#"return docs["my_fn"]"#).eval()?;
//! assert_eq!(doc, "(function) Does something useful");
//! # Ok(())
//! # }
//! ```

/// Documentation entry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaDocTyp {
    /// A callable function.
    Function,
    /// A namespace or module containing functions.
    Scope,
}

/// Documentation entry for a Lua function or scope.
#[derive(Debug, Clone)]
pub struct LuaDoc {
    /// Fully qualified name (e.g., "os.time" or "string").
    pub name: String,
    /// Type of the documented item.
    pub typ: LuaDocTyp,
    /// Human-readable description.
    pub description: String,
}

/// Registers documentation into the Lua `docs` global table.
///
/// Creates or updates the `docs` table with an entry for the given name. The entry
/// is formatted as `"(<type>) <description>"`.
///
/// # Example
///
/// ```
/// use onetool::runtime::docs::{register, LuaDoc, LuaDocTyp};
///
/// # fn example() -> mlua::Result<()> {
/// let lua = mlua::Lua::new();
///
/// register(&lua, &LuaDoc {
///     name: "math".to_string(),
///     typ: LuaDocTyp::Scope,
///     description: "Mathematical functions".to_string(),
/// })?;
///
/// register(&lua, &LuaDoc {
///     name: "math.sqrt".to_string(),
///     typ: LuaDocTyp::Function,
///     description: "Returns the square root of a number".to_string(),
/// })?;
/// # Ok(())
/// # }
/// ```
pub fn register(lua: &mlua::Lua, doc: &LuaDoc) -> mlua::Result<()> {
    let typ_str = match doc.typ {
        LuaDocTyp::Function => "function",
        LuaDocTyp::Scope => "scope",
    };

    let content = format!("({}) {}", typ_str, doc.description);

    // Initialize docs table if needed
    lua.load("docs = docs or {}").exec()?;

    // Set the doc entry directly from Rust to avoid escaping issues
    let docs: mlua::Table = lua.globals().get("docs")?;
    docs.set(doc.name.as_str(), content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_function_doc() {
        let lua = mlua::Lua::new();
        let doc = LuaDoc {
            name: "test_fn".to_string(),
            typ: LuaDocTyp::Function,
            description: "A test function".to_string(),
        };

        register(&lua, &doc).unwrap();

        // Verify docs table exists
        let docs_exists: bool = lua.load("return docs ~= nil").eval().unwrap();
        assert!(docs_exists);

        // Verify the doc was registered
        let doc_content: String = lua.load(r#"return docs["test_fn"]"#).eval().unwrap();
        assert_eq!(doc_content, "(function) A test function");
    }

    #[test]
    fn register_scope_doc() {
        let lua = mlua::Lua::new();
        let doc = LuaDoc {
            name: "test_scope".to_string(),
            typ: LuaDocTyp::Scope,
            description: "A test scope".to_string(),
        };

        register(&lua, &doc).unwrap();

        // Verify the doc was registered with scope type
        let doc_content: String = lua.load(r#"return docs["test_scope"]"#).eval().unwrap();
        assert_eq!(doc_content, "(scope) A test scope");
    }

    #[test]
    fn register_multiple_docs() {
        let lua = mlua::Lua::new();

        let docs = vec![
            LuaDoc {
                name: "fn1".to_string(),
                typ: LuaDocTyp::Function,
                description: "First function".to_string(),
            },
            LuaDoc {
                name: "fn2".to_string(),
                typ: LuaDocTyp::Function,
                description: "Second function".to_string(),
            },
            LuaDoc {
                name: "scope1".to_string(),
                typ: LuaDocTyp::Scope,
                description: "First scope".to_string(),
            },
        ];

        for doc in &docs {
            register(&lua, doc).unwrap();
        }

        // Verify all docs are registered
        let fn1_exists: bool = lua.load(r#"return docs["fn1"] ~= nil"#).eval().unwrap();
        let fn2_exists: bool = lua.load(r#"return docs["fn2"] ~= nil"#).eval().unwrap();
        let scope1_exists: bool = lua.load(r#"return docs["scope1"] ~= nil"#).eval().unwrap();

        assert!(fn1_exists);
        assert!(fn2_exists);
        assert!(scope1_exists);

        // Verify doc count
        let doc_count: usize = lua
            .load(
                r#"
                local count = 0
                for _ in pairs(docs) do count = count + 1 end
                return count
            "#,
            )
            .eval()
            .unwrap();
        assert_eq!(doc_count, 3);
    }

    #[test]
    fn register_doc_with_special_characters() {
        let lua = mlua::Lua::new();
        let doc = LuaDoc {
            name: "special_fn".to_string(),
            typ: LuaDocTyp::Function,
            description: "Description with \"quotes\" and 'apostrophes'".to_string(),
        };

        register(&lua, &doc).unwrap();

        let doc_content: String = lua.load(r#"return docs["special_fn"]"#).eval().unwrap();
        assert!(doc_content.contains("quotes"));
        assert!(doc_content.contains("apostrophes"));
    }

    #[test]
    fn register_overwrites_existing_doc() {
        let lua = mlua::Lua::new();

        let doc1 = LuaDoc {
            name: "overwrite_test".to_string(),
            typ: LuaDocTyp::Function,
            description: "First description".to_string(),
        };

        let doc2 = LuaDoc {
            name: "overwrite_test".to_string(),
            typ: LuaDocTyp::Scope,
            description: "Second description".to_string(),
        };

        register(&lua, &doc1).unwrap();
        register(&lua, &doc2).unwrap();

        // Verify the second doc overwrote the first
        let doc_content: String = lua.load(r#"return docs["overwrite_test"]"#).eval().unwrap();
        assert!(doc_content.contains("(scope)"));
        assert!(doc_content.contains("Second description"));
        assert!(!doc_content.contains("First description"));
    }

    #[test]
    fn doc_format_consistency() {
        let lua = mlua::Lua::new();
        let doc = LuaDoc {
            name: "format_test".to_string(),
            typ: LuaDocTyp::Function,
            description: "Test description".to_string(),
        };

        register(&lua, &doc).unwrap();

        let doc_content: String = lua.load(r#"return docs["format_test"]"#).eval().unwrap();

        // Verify format structure: "(<type>) <description>"
        assert_eq!(doc_content, "(function) Test description");
    }
}
