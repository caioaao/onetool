use crate::runtime::docs::{self, LuaDoc, LuaDocTyp};
use crate::runtime::policy;
use std::sync::Arc;

pub fn install_onetool_api<P: policy::Policy + 'static>(
    lua: &mlua::Lua,
    policy: Arc<P>,
) -> mlua::Result<()> {
    Builder::new(lua)?.with_require(policy)?.finish()?;

    Ok(())
}

// fn with_require(lua: &mlua::Lua, policy: Arc<P>)
struct Builder<'lua> {
    lua: &'lua mlua::Lua,
    table: mlua::Table,
}

impl<'lua> Builder<'lua> {
    pub fn new(lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let table = lua.create_table()?;
        Ok(Self { lua, table })
    }

    pub fn with_require<P: policy::Policy + 'static>(self, policy: Arc<P>) -> mlua::Result<Self> {
        let require_fn = self.lua.create_function(move |lua, pkg_name: String| {
            let decision = policy.check_access(
                &policy::Caller::Agent,
                &policy::Action::LoadPackage(pkg_name.clone()),
            );

            if let policy::AccessDecision::Deny(reason) = decision {
                eprintln!("Access denied: {}", reason);
                return Ok(mlua::Value::Nil);
            }

            match lua.globals().get::<mlua::Function>("require") {
                Ok(require_fn) => match require_fn.call::<mlua::Value>(pkg_name) {
                    Ok(module) => return Ok(module),
                    Err(_) => return Ok(mlua::Value::Nil),
                },
                Err(_) => return Ok(mlua::Value::Nil),
            }
        })?;

        self.table.set("require", require_fn)?;

        docs::register(
            self.lua,
            &LuaDoc {
                name: "onetool.require".to_string(),
                typ: LuaDocTyp::Function,
                description: "Load a package with access control. Returns nil if denied."
                    .to_string(),
            },
        )?;

        Ok(self)
    }

    pub fn finish(self) -> mlua::Result<()> {
        self.lua.globals().set("onetool", self.table)?;

        // Register documentation
        docs::register(
            self.lua,
            &LuaDoc {
                name: "onetool".to_string(),
                typ: LuaDocTyp::Scope,
                description: "OneTool runtime utilities".to_string(),
            },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::policy::{AccessDecision, Action, Caller, Policy};

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

    // ============================================================================
    // Test Helper Functions
    // ============================================================================

    /// Creates a simple test package (a Lua table with a test function)
    fn create_test_package(lua: &mlua::Lua, name: &str) -> mlua::Result<mlua::Value> {
        let package = lua.create_table()?;
        let name_owned = name.to_string();
        let test_fn =
            lua.create_function(move |_lua, ()| Ok(format!("Hello from {}", name_owned)))?;
        package.set("test", test_fn)?;
        Ok(mlua::Value::Table(package))
    }

    // ============================================================================
    // Installation Tests
    // ============================================================================

    #[test]
    fn install_creates_onetool_global() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        install_onetool_api(&lua, policy).unwrap();

        // Verify onetool global exists and is a table
        let onetool_exists: bool = lua.load("return onetool ~= nil").eval().unwrap();
        assert!(onetool_exists);

        let onetool_type: String = lua.load("return type(onetool)").eval().unwrap();
        assert_eq!(onetool_type, "table");
    }

    #[test]
    fn install_creates_require_function() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        install_onetool_api(&lua, policy).unwrap();

        // Verify onetool.require is a function
        let require_exists: bool = lua.load("return onetool.require ~= nil").eval().unwrap();
        assert!(require_exists);

        let require_type: String = lua.load("return type(onetool.require)").eval().unwrap();
        assert_eq!(require_type, "function");
    }

    #[test]
    fn install_registers_onetool_documentation() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        install_onetool_api(&lua, policy).unwrap();

        // Check docs["onetool"] exists
        let doc_exists: bool = lua.load(r#"return docs["onetool"] ~= nil"#).eval().unwrap();
        assert!(doc_exists);

        // Verify format: "(scope) OneTool runtime utilities"
        let doc_content: String = lua.load(r#"return docs["onetool"]"#).eval().unwrap();
        assert!(doc_content.contains("(scope)"));
        assert!(doc_content.contains("OneTool runtime utilities"));
    }

    #[test]
    fn install_registers_require_documentation() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);
        install_onetool_api(&lua, policy).unwrap();

        // Check docs["onetool.require"] exists
        let doc_exists: bool = lua
            .load(r#"return docs["onetool.require"] ~= nil"#)
            .eval()
            .unwrap();
        assert!(doc_exists);

        // Verify format includes "function" and access control info
        let doc_content: String = lua
            .load(r#"return docs["onetool.require"]"#)
            .eval()
            .unwrap();
        assert!(doc_content.contains("(function)"));
        assert!(doc_content.contains("access control"));
    }

    // ============================================================================
    // Policy Integration Tests
    // ============================================================================

    #[test]
    fn require_returns_nil_when_policy_denies() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(DenyPolicy);

        install_onetool_api(&lua, policy).unwrap();

        // Verify that when policy denies, nil is returned
        let result: mlua::Value = lua.load(r#"return onetool.require("io")"#).eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn falls_back_to_standard_require() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Create user package in package.loaded
        lua.load(
            r#"
            package = package or {}
            package.loaded = package.loaded or {}
            package.loaded["userlib"] = { name = "userlib", version = "1.0" }
        "#,
        )
        .exec()
        .unwrap();

        // Setup standard require function
        lua.load(
            r#"
            require = function(name)
                return package.loaded[name]
            end
        "#,
        )
        .exec()
        .unwrap();

        install_onetool_api(&lua, policy).unwrap();

        // Verify fallback to standard require works
        let name: String = lua
            .load(r#"return onetool.require("userlib").name"#)
            .eval()
            .unwrap();
        assert_eq!(name, "userlib");
    }

    #[test]
    fn fallback_error_returns_nil() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Setup standard require that fails for unknown packages
        lua.load(
            r#"
            require = function(name)
                error("module '" .. name .. "' not found")
            end
        "#,
        )
        .exec()
        .unwrap();

        install_onetool_api(&lua, policy).unwrap();

        // Verify error in fallback returns nil instead of propagating
        let result: mlua::Value = lua
            .load(r#"return onetool.require("nonexistent")"#)
            .eval()
            .unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn missing_require_function_returns_nil() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Don't setup standard require (it's nil)
        lua.globals().set("require", mlua::Value::Nil).unwrap();

        install_onetool_api(&lua, policy).unwrap();

        // Verify graceful handling when require is missing
        let result: mlua::Value = lua
            .load(r#"return onetool.require("anything")"#)
            .eval()
            .unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    // ============================================================================
    // Builder Pattern Tests
    // ============================================================================

    #[test]
    fn builder_completes_successfully() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        // Verify Builder chain completes
        let result = Builder::new(&lua)
            .and_then(|b| b.with_require(policy))
            .and_then(|b| b.finish());

        assert!(result.is_ok());
    }

    #[test]
    fn builder_creates_callable_require() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        Builder::new(&lua)
            .and_then(|b| b.with_require(policy))
            .and_then(|b| b.finish())
            .unwrap();

        // Verify onetool.require is callable
        let is_function: bool = lua
            .load(r#"return type(onetool.require) == "function""#)
            .eval()
            .unwrap();
        assert!(is_function);
    }

    #[test]
    fn multiple_installs_overwrite_previous() {
        let lua = mlua::Lua::new();

        // First install with DenyPolicy
        let deny_policy = Arc::new(DenyPolicy);
        install_onetool_api(&lua, deny_policy).unwrap();

        // Verify denial
        let result1: mlua::Value = lua.load(r#"return onetool.require("io")"#).eval().unwrap();
        assert!(matches!(result1, mlua::Value::Nil));

        // Second install with AllowPolicy
        let allow_policy = Arc::new(AllowPolicy);
        install_onetool_api(&lua, allow_policy).unwrap();

        // Verify second install overwrites first
        let result2: mlua::Value = lua.load(r#"return onetool.require("io")"#).eval().unwrap();
        assert!(matches!(result2, mlua::Value::Table(_)));
    }

    // ============================================================================
    // 6. Edge Cases
    // ============================================================================

    #[test]
    fn empty_package_name_handled_gracefully() {
        let lua = mlua::Lua::new();
        let policy = Arc::new(AllowPolicy);

        install_onetool_api(&lua, policy).unwrap();

        // Verify empty string doesn't panic
        let result: mlua::Value = lua.load(r#"return onetool.require("")"#).eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }
}
