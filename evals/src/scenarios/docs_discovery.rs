use super::Scenario;
use crate::runner::Transcript;
use crate::scoring::{self, ScoreResult};
use onetool::Repl;
use onetool::runtime::docs::{self, LuaDoc, LuaDocTyp};

// ---------------------------------------------------------------------------
// S1: Discover and use a custom function via docs
// ---------------------------------------------------------------------------

pub struct DiscoverAndUse;

impl Scenario for DiscoverAndUse {
    fn id(&self) -> &str {
        "docs_discover_custom_fn"
    }

    fn tags(&self) -> &[&str] {
        &["core", "docs"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        repl.with_runtime(|lua| {
            let http_fetch = lua.create_function(|_, url: String| {
                Ok(format!(
                    "{{\"status\": 200, \"body\": \"Hello from {}\"}}",
                    url
                ))
            })?;
            lua.globals().set("http_fetch", http_fetch)?;

            docs::register(
                lua,
                &LuaDoc {
                    name: "http_fetch".to_string(),
                    typ: LuaDocTyp::Function,
                    description:
                        "Fetches a URL. Usage: http_fetch(url) -> JSON string with status and body"
                            .to_string(),
                },
            )?;

            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Fetch the content from https://example.com and tell me what the response body says."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let queried_docs = scoring::docs_queried(transcript);
        let called_fn = scoring::transcript_contains_lua(transcript, "http_fetch");
        let has_answer = scoring::final_answer_contains_ci(transcript, "Hello from");

        let mut score = 0.0;
        let mut details = Vec::new();

        if queried_docs {
            score += 0.3;
            details.push("queried docs");
        } else {
            details.push("did NOT query docs");
        }

        if called_fn {
            score += 0.4;
            details.push("called http_fetch");
        } else {
            details.push("did NOT call http_fetch");
        }

        if has_answer {
            score += 0.3;
            details.push("correct answer");
        } else {
            details.push("wrong/missing answer");
        }

        ScoreResult {
            passed: queried_docs && called_fn && has_answer,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "Did the model discover the http_fetch function through the docs global, call it correctly, and report the result? Score higher if it checked docs first before guessing.",
        )
    }
}

// ---------------------------------------------------------------------------
// S2: Enumerate all custom functions
// ---------------------------------------------------------------------------

pub struct EnumerateAll;

impl Scenario for EnumerateAll {
    fn id(&self) -> &str {
        "docs_enumerate_all"
    }

    fn tags(&self) -> &[&str] {
        &["docs"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        repl.with_runtime(|lua| {
            for (name, desc) in [
                ("fetch_url", "Fetches a URL"),
                ("hash_md5", "Computes MD5 hash"),
                ("send_email", "Sends an email"),
                ("resize_image", "Resizes an image"),
            ] {
                let f = lua.create_function(|_, ()| Ok("stub"))?;
                lua.globals().set(name, f)?;
                docs::register(
                    lua,
                    &LuaDoc {
                        name: name.to_string(),
                        typ: LuaDocTyp::Function,
                        description: desc.to_string(),
                    },
                )?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "What custom tools are available in this environment? List all of them."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let queried_docs = scoring::docs_queried(transcript);
        let fns = ["fetch_url", "hash_md5", "send_email", "resize_image"];
        let mentioned: Vec<&str> = fns
            .iter()
            .filter(|f| scoring::final_answer_contains_ci(transcript, f))
            .copied()
            .collect();

        let ratio = mentioned.len() as f64 / fns.len() as f64;

        ScoreResult {
            passed: queried_docs && mentioned.len() == fns.len(),
            score: if queried_docs { ratio } else { ratio * 0.5 },
            details: format!(
                "queried docs: {}, mentioned {}/{}: {:?}",
                queried_docs,
                mentioned.len(),
                fns.len(),
                mentioned
            ),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "Did the model enumerate all 4 custom functions (fetch_url, hash_md5, send_email, resize_image) by checking the docs global? Score based on completeness and accuracy.",
        )
    }
}

// ---------------------------------------------------------------------------
// S3: Implicit discovery (no hint about docs)
// ---------------------------------------------------------------------------

pub struct ImplicitDiscovery;

impl Scenario for ImplicitDiscovery {
    fn id(&self) -> &str {
        "docs_implicit_discovery"
    }

    fn tags(&self) -> &[&str] {
        &["docs"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        repl.with_runtime(|lua| {
            let translate =
                lua.create_function(|_, text: String| Ok(format!("[translated] {}", text)))?;
            lua.globals().set("translate", translate)?;

            docs::register(
                lua,
                &LuaDoc {
                    name: "translate".to_string(),
                    typ: LuaDocTyp::Function,
                    description:
                        "Translates text to English. Usage: translate(text) -> translated string"
                            .to_string(),
                },
            )?;

            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Translate the text 'Bonjour le monde' to English."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let queried_docs = scoring::docs_queried(transcript);
        let called_fn = scoring::transcript_contains_lua(transcript, "translate");

        let mut score = 0.0;
        let mut details = Vec::new();

        if queried_docs {
            score += 0.5;
            details.push("discovered docs unprompted");
        } else {
            details.push("did NOT check docs");
        }

        if called_fn {
            score += 0.5;
            details.push("called translate");
        } else {
            details.push("did NOT call translate");
        }

        ScoreResult {
            passed: queried_docs && called_fn,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "The model was asked to translate text without any hint about available functions. Did it proactively check the docs global and discover the translate function? Score higher for unprompted discovery.",
        )
    }
}

// ---------------------------------------------------------------------------
// S4: Empty docs (no custom functions)
// ---------------------------------------------------------------------------

pub struct EmptyDocs;

impl Scenario for EmptyDocs {
    fn id(&self) -> &str {
        "docs_empty"
    }

    fn tags(&self) -> &[&str] {
        &["docs"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        // No custom functions registered
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "What custom functions are available in this environment beyond the standard Lua libraries?"
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let queried_docs = scoring::docs_queried(transcript);
        // The model should NOT hallucinate custom functions.
        // The docs table will have built-in docs (os, string, etc.) but no custom ones.
        let hallucinated = ["fetch", "http", "email", "image", "translate", "hash"]
            .iter()
            .any(|f| scoring::final_answer_contains_ci(transcript, f));

        let mut score = 0.0;
        let mut details = Vec::new();

        if queried_docs {
            score += 0.5;
            details.push("checked docs");
        } else {
            details.push("did NOT check docs");
        }

        if !hallucinated {
            score += 0.5;
            details.push("no hallucinated functions");
        } else {
            details.push("HALLUCINATED custom functions");
        }

        ScoreResult {
            passed: queried_docs && !hallucinated,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "When no custom functions are registered, does the model correctly report that there are no custom functions (beyond standard library docs)? It should not hallucinate functions that don't exist.",
        )
    }
}
