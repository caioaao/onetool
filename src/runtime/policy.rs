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
pub enum AccessDecision {
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
    fn check_access(&self, scope: &Caller, action: &Action) -> AccessDecision;
}

/// Strict policy that denies all access requests
///
/// This is the default policy that blocks all attempts to load packages
/// or perform restricted operations.
pub struct DenyAllPolicy;

impl Policy for DenyAllPolicy {
    fn check_access(&self, _: &Caller, _: &Action) -> AccessDecision {
        AccessDecision::Deny("Access denied by strict policy".to_string())
    }
}

/// Permissive policy that allows specific packages via an allowlist
///
/// This policy grants access only to packages explicitly listed in the allowlist.
/// All other packages are denied.
///
/// # Example
/// ```
/// use onetool::runtime::policy::WhiteListPolicy;
///
/// let policy = WhiteListPolicy::new(&["io", "os"]);
/// ```
pub struct WhiteListPolicy {
    allowed_packages: std::collections::HashSet<String>,
}

impl WhiteListPolicy {
    /// Creates a new permissive policy with the given allowlist
    ///
    /// # Arguments
    /// * `allowed` - Slice of package names that should be allowed
    pub fn new(allowed: &[&str]) -> Self {
        Self {
            allowed_packages: allowed.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl Policy for WhiteListPolicy {
    fn check_access(&self, _scope: &Caller, action: &Action) -> AccessDecision {
        match action {
            Action::LoadPackage(name) => {
                if self.allowed_packages.contains(name) {
                    AccessDecision::Allow
                } else {
                    AccessDecision::Deny(format!("Package '{}' not in allowlist", name))
                }
            }
            Action::CallFunction { name: _, args: _ } => AccessDecision::Allow,
        }
    }
}
