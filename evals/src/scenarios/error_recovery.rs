use super::Scenario;
use crate::runner::Transcript;
use crate::scoring::{self, ScoreResult};
use onetool::Repl;

// ---------------------------------------------------------------------------
// S9: Policy denial recovery
// ---------------------------------------------------------------------------

pub struct PolicyDenial;

impl Scenario for PolicyDenial {
    fn id(&self) -> &str {
        "error_policy_denial"
    }

    fn tags(&self) -> &[&str] {
        &["core", "error"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        // Default DenyAllPolicy is already applied by Repl::new()
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Read the contents of the file '/etc/hostname' and tell me what it says."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let attempted_io = scoring::transcript_contains_lua(transcript, "io.open")
            || scoring::transcript_contains_lua(transcript, "io.read")
            || scoring::transcript_contains_lua(transcript, "io.lines");

        // Should detect nil return and explain the limitation
        let explained_limitation = scoring::final_answer_contains_ci(transcript, "denied")
            || scoring::final_answer_contains_ci(transcript, "not available")
            || scoring::final_answer_contains_ci(transcript, "not accessible")
            || scoring::final_answer_contains_ci(transcript, "sandbox")
            || scoring::final_answer_contains_ci(transcript, "restricted")
            || scoring::final_answer_contains_ci(transcript, "nil")
            || scoring::final_answer_contains_ci(transcript, "disabled")
            || scoring::final_answer_contains_ci(transcript, "can't")
            || scoring::final_answer_contains_ci(transcript, "cannot")
            || scoring::final_answer_contains_ci(transcript, "unable");

        // Should NOT retry the same blocked call repeatedly
        let io_open_count = transcript
            .lua_snippets
            .iter()
            .filter(|s| s.contains("io.open"))
            .count();
        let no_excessive_retry = io_open_count <= 2;

        let mut score = 0.0;
        let mut details: Vec<&str> = Vec::new();

        if attempted_io {
            score += 0.3;
            details.push("attempted file I/O");
        } else {
            details.push("did NOT attempt file I/O");
        }

        if explained_limitation {
            score += 0.4;
            details.push("explained limitation");
        } else {
            details.push("did NOT explain limitation");
        }

        if no_excessive_retry {
            score += 0.3;
            details.push("no excessive retries");
        } else {
            details.push("excessive retries");
        }

        ScoreResult {
            passed: explained_limitation && no_excessive_retry,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "The model was asked to read a file, but io.open is blocked by policy (returns nil). Did it detect the nil, explain the limitation clearly, and avoid retrying the same blocked call?",
        )
    }
}

// ---------------------------------------------------------------------------
// S10: Runtime error recovery
// ---------------------------------------------------------------------------

pub struct RuntimeError;

impl Scenario for RuntimeError {
    fn id(&self) -> &str {
        "error_runtime_recovery"
    }

    fn tags(&self) -> &[&str] {
        &["error"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        repl.with_runtime(|lua| {
            // A function that errors on first call but we'll track attempts
            let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let cc = call_count.clone();
            let flaky_fn = lua.create_function(move |_, input: String| {
                let count = cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count == 0 {
                    Err(mlua::Error::RuntimeError(
                        "Connection timeout - service temporarily unavailable".to_string(),
                    ))
                } else {
                    Ok(format!("processed: {}", input.to_uppercase()))
                }
            })?;
            lua.globals().set("process_text", flaky_fn)?;

            onetool::runtime::docs::register(
                lua,
                &onetool::runtime::docs::LuaDoc {
                    name: "process_text".to_string(),
                    typ: onetool::runtime::docs::LuaDocTyp::Function,
                    description: "Processes text. Usage: process_text(text) -> processed string. May fail transiently.".to_string(),
                },
            )?;

            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Use the process_text function to process the text 'hello world'. If it fails, try again or handle the error gracefully."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let called_fn = scoring::transcript_contains_lua(transcript, "process_text");
        // Should use pcall or try/catch pattern
        let used_error_handling = scoring::transcript_contains_lua(transcript, "pcall")
            || scoring::transcript_contains_lua(transcript, "xpcall");
        let got_result = scoring::final_answer_contains_ci(transcript, "HELLO WORLD")
            || scoring::tool_response_contains(transcript, "HELLO WORLD");

        let mut score = 0.0;
        let mut details = Vec::new();

        if called_fn {
            score += 0.2;
            details.push("called process_text");
        }

        if used_error_handling {
            score += 0.3;
            details.push("used pcall/xpcall");
        } else if transcript.lua_snippets.len() > 1 {
            score += 0.15;
            details.push("retried after error (no pcall)");
        } else {
            details.push("no error handling");
        }

        if got_result {
            score += 0.5;
            details.push("got correct result");
        } else {
            details.push("no correct result");
        }

        ScoreResult {
            passed: called_fn && got_result,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "A flaky function errors on first call but succeeds on retry. Did the model handle the error and eventually get the result? Was the error recovery strategy reasonable?",
        )
    }
}

// ---------------------------------------------------------------------------
// S11: Syntax self-correction
// ---------------------------------------------------------------------------

pub struct SyntaxSelfCorrection;

impl Scenario for SyntaxSelfCorrection {
    fn id(&self) -> &str {
        "error_syntax_self_correction"
    }

    fn tags(&self) -> &[&str] {
        &["error"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Create a list of numbers from 1 to 10, filter out the even ones, and return the sum of the remaining odd numbers. Use the standard approach you'd use in any programming language."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        // Sum of odd numbers 1-10: 1+3+5+7+9 = 25
        let correct_answer = scoring::final_answer_contains(transcript, "25");
        let used_repl = !transcript.lua_snippets.is_empty();
        let had_errors = !transcript.lua_errors.is_empty();
        let eventually_succeeded = correct_answer && used_repl;

        let mut score = 0.0;
        let mut details = Vec::new();

        if used_repl {
            score += 0.2;
            details.push("used REPL");
        }

        if correct_answer {
            score += 0.5;
            details.push("correct answer (25)");
        } else {
            details.push("wrong/missing answer");
        }

        if had_errors && eventually_succeeded {
            score += 0.3;
            details.push("recovered from syntax errors");
        } else if !had_errors && eventually_succeeded {
            score += 0.3;
            details.push("no errors needed recovery");
        } else if had_errors {
            details.push("had errors but didn't recover");
        }

        ScoreResult {
            passed: eventually_succeeded,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "The prompt is designed to potentially trigger Python-isms (list comprehensions, etc.) in a Lua environment. Did the model write valid Lua? If it made syntax errors, did it self-correct? The correct answer is 25.",
        )
    }
}

// ---------------------------------------------------------------------------
// S12: Probe before use
// ---------------------------------------------------------------------------

pub struct ProbeBeforeUse;

impl Scenario for ProbeBeforeUse {
    fn id(&self) -> &str {
        "error_probe_before_use"
    }

    fn tags(&self) -> &[&str] {
        &["error"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        // Default sandbox: io.open returns nil
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Save the text 'Hello, World!' to a file called output.txt if file I/O is available. If not, just print the text instead."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        // Should probe for io.open availability
        let probed = scoring::transcript_contains_lua(transcript, "if io")
            || scoring::transcript_contains_lua(transcript, "io.open")
            || scoring::transcript_contains_lua(transcript, "pcall");

        // Should fall back to print since io.open returns nil
        let fell_back_to_print = scoring::transcript_contains_lua(transcript, "print");

        // Should mention that file I/O was not available
        let explained = scoring::final_answer_contains_ci(transcript, "not available")
            || scoring::final_answer_contains_ci(transcript, "not accessible")
            || scoring::final_answer_contains_ci(transcript, "print")
            || scoring::final_answer_contains_ci(transcript, "unable")
            || scoring::final_answer_contains_ci(transcript, "disabled")
            || scoring::final_answer_contains_ci(transcript, "sandbox");

        let mut score = 0.0;
        let mut details = Vec::new();

        if probed {
            score += 0.4;
            details.push("probed io availability");
        } else {
            details.push("did NOT probe io");
        }

        if fell_back_to_print {
            score += 0.3;
            details.push("fell back to print");
        } else {
            details.push("did NOT fall back to print");
        }

        if explained {
            score += 0.3;
            details.push("explained outcome");
        } else {
            details.push("no explanation");
        }

        ScoreResult {
            passed: probed && fell_back_to_print,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "The model was asked to save to file if possible, otherwise print. File I/O is blocked. Did it probe for io availability (e.g., 'if io.open then'), fall back to print, and explain what happened?",
        )
    }
}
