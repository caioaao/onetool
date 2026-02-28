#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    Agent,
    Package(String),
}

#[derive(Debug, Clone)]
pub enum Action {
    LoadPackage(String),
    CallFunction {
        name: String,
        args: mlua::MultiValue,
    },
}

/// Decision result from an access policy check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Access is granted
    Allow,
    /// Access is denied with a reason
    Deny(String),
}

/// Policy controls access to dangerous/restricted APIs
///
/// Implementations of this trait determine whether specific actions
/// (like loading packages) should be allowed in the sandboxed environment.
pub trait Policy: Send + Sync {
    /// Check if an action should be allowed
    ///
    /// # Arguments
    /// * `caller` - The caller
    /// * `action` - The action being requested
    ///
    /// # Returns
    /// `AccessDecision::Allow` if the action should be permitted,
    /// `AccessDecision::Deny(reason)` otherwise
    fn check_access(&self, scope: &Caller, action: &Action) -> Decision;
}

/// Strict policy that denies all access requests
///
/// This is the default policy that blocks all attempts to load packages
/// or perform restricted operations.
pub struct DenyAllPolicy;

impl Policy for DenyAllPolicy {
    fn check_access(&self, _: &Caller, _: &Action) -> Decision {
        Decision::Deny("Access denied by strict policy".to_string())
    }
}

/// Permissive policy that allows **Unsafe** functions to execute
///
/// This policy grants access to policy-controlled **Unsafe** functions (like `os.execute`,
/// `io.open`, etc.), while **Forbidden** functions (like `debug`, `coroutine`, `package`)
/// remain completely blocked (set to nil).
///
/// # Function Categories
/// - **Safe** functions: Always available (no policy check needed)
/// - **Unsafe** functions: Allowed by this policy (normally require approval)
/// - **Forbidden** functions: Still blocked (removed from environment)
///
/// # Security Implications
/// **WARNING**: This policy bypasses access control for Unsafe functions and should
/// only be used in trusted environments:
/// - During development and testing
/// - In completely trusted environments
/// - When you need filesystem/process access but still want Forbidden APIs blocked
///
/// # Example
/// ```ignore
/// use onetool::runtime::sandbox;
/// use onetool::runtime::sandbox::policy::DangerousAllowAllPolicy;
///
/// let lua = mlua::Lua::new();
/// sandbox::apply_with_policy(&lua, DangerousAllowAllPolicy)?;
///
/// // Unsafe functions now work
/// lua.load("os.execute('echo hello')").exec()?;  // ✓ Allowed
///
/// // Forbidden functions are still blocked
/// let result: mlua::Value = lua.load("return debug").eval()?;
/// assert!(matches!(result, mlua::Value::Nil));  // ✓ Still nil
/// ```
pub struct DangerousAllowAllPolicy;

impl Policy for DangerousAllowAllPolicy {
    fn check_access(&self, _: &Caller, _: &Action) -> Decision {
        Decision::Allow
    }
}
