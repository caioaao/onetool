pub mod docs_discovery;
pub mod error_recovery;
pub mod general;
pub mod repl_usage;

use crate::runner::Transcript;
use crate::scoring::ScoreResult;
use onetool::Repl;

/// A single eval scenario.
pub trait Scenario: Send + Sync {
    /// Unique identifier (e.g. "docs_discover_custom_fn").
    fn id(&self) -> &str;

    /// Tags for filtering (e.g. ["core", "docs"]).
    fn tags(&self) -> &[&str];

    /// Set up the Repl for this scenario (register functions, globals, docs, policies).
    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>>;

    /// The user prompt to send to the model.
    fn user_prompt(&self) -> &str;

    /// Deterministic scoring of the transcript.
    fn score(&self, transcript: &Transcript) -> ScoreResult;

    /// Optional prompt for LLM-as-judge evaluation. Return None to skip.
    fn judge_prompt(&self) -> Option<&str> {
        None
    }
}

/// Returns all registered scenarios.
pub fn all() -> Vec<Box<dyn Scenario>> {
    vec![
        // docs discovery
        Box::new(docs_discovery::DiscoverAndUse),
        Box::new(docs_discovery::EnumerateAll),
        Box::new(docs_discovery::ImplicitDiscovery),
        Box::new(docs_discovery::EmptyDocs),
        // repl usage
        Box::new(repl_usage::StatefulComputation),
        Box::new(repl_usage::ReachForTool),
        Box::new(repl_usage::LuaSyntax),
        Box::new(repl_usage::DataProcessing),
        // error recovery
        Box::new(error_recovery::PolicyDenial),
        Box::new(error_recovery::RuntimeError),
        Box::new(error_recovery::SyntaxSelfCorrection),
        Box::new(error_recovery::ProbeBeforeUse),
        // general
        Box::new(general::NeedleInHaystack),
        Box::new(general::TwinPrimes),
    ]
}
