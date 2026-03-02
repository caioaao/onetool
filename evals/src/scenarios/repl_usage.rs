use super::Scenario;
use crate::runner::Transcript;
use crate::scoring::{self, ScoreResult};
use onetool::Repl;

// ---------------------------------------------------------------------------
// S5: Stateful computation (define factorial, compute 10!)
// ---------------------------------------------------------------------------

pub struct StatefulComputation;

impl Scenario for StatefulComputation {
    fn id(&self) -> &str {
        "repl_stateful_computation"
    }

    fn tags(&self) -> &[&str] {
        &["repl"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Define a factorial function, then compute 10! (ten factorial). Tell me the result."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let has_function_def = scoring::transcript_contains_lua(transcript, "function")
            && scoring::transcript_contains_lua_ci(transcript, "factorial");
        let correct_answer = scoring::final_answer_contains(transcript, "3628800");
        let used_repl = !transcript.lua_snippets.is_empty();

        let mut score = 0.0;
        let mut details = Vec::new();

        if has_function_def {
            score += 0.3;
            details.push("defined factorial function");
        } else {
            details.push("no factorial function definition");
        }

        if correct_answer {
            score += 0.5;
            details.push("correct answer (3628800)");
        } else {
            details.push("wrong/missing answer");
        }

        if used_repl {
            score += 0.2;
            details.push("used REPL");
        }

        ScoreResult {
            passed: has_function_def && correct_answer,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "Did the model define a factorial function and correctly compute 10! = 3628800 using the Lua REPL? Was the approach efficient?",
        )
    }
}

// ---------------------------------------------------------------------------
// S6: Reach for tool (50th Fibonacci)
// ---------------------------------------------------------------------------

pub struct ReachForTool;

impl Scenario for ReachForTool {
    fn id(&self) -> &str {
        "repl_reach_for_tool"
    }

    fn tags(&self) -> &[&str] {
        &["core", "repl"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "What is the 50th Fibonacci number? (Starting from fib(1)=1, fib(2)=1)"
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let used_repl = !transcript.lua_snippets.is_empty();
        // 50th Fibonacci: 12586269025
        let correct_answer = scoring::final_answer_contains(transcript, "12586269025");

        let mut score = 0.0;
        let mut details = Vec::new();

        if used_repl {
            score += 0.4;
            details.push("used Lua REPL (didn't guess)");
        } else {
            details.push("did NOT use REPL");
        }

        if correct_answer {
            score += 0.6;
            details.push("correct answer (12586269025)");
        } else {
            details.push("wrong/missing answer");
        }

        ScoreResult {
            passed: used_repl && correct_answer,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "The 50th Fibonacci number is 12586269025. Did the model use the Lua REPL to compute this rather than guessing? Was the Lua code correct?",
        )
    }
}

// ---------------------------------------------------------------------------
// S7: Lua syntax (pattern matching, not regex)
// ---------------------------------------------------------------------------

pub struct LuaSyntax;

impl Scenario for LuaSyntax {
    fn id(&self) -> &str {
        "repl_lua_syntax"
    }

    fn tags(&self) -> &[&str] {
        &["repl"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        repl.with_runtime(|lua| {
            lua.globals().set(
                "sample_text",
                "Contact us at support@example.com or sales@company.org for inquiries.",
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Extract all email addresses from the sample_text variable."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let used_repl = !transcript.lua_snippets.is_empty();
        // Should use Lua pattern matching, not regex syntax
        let used_lua_patterns = scoring::transcript_contains_lua(transcript, "gmatch")
            || scoring::transcript_contains_lua(transcript, "match")
            || scoring::transcript_contains_lua(transcript, "find");

        let found_emails = scoring::final_answer_contains_ci(transcript, "support@example.com")
            && scoring::final_answer_contains_ci(transcript, "sales@company.org");

        let no_errors = scoring::no_lua_errors(transcript);

        let mut score = 0.0;
        let mut details = Vec::new();

        if used_lua_patterns {
            score += 0.3;
            details.push("used Lua patterns");
        } else if used_repl {
            score += 0.1;
            details.push("used REPL but unclear patterns");
        } else {
            details.push("did not use REPL");
        }

        if found_emails {
            score += 0.4;
            details.push("found both emails");
        } else {
            details.push("missing emails");
        }

        if no_errors {
            score += 0.3;
            details.push("no syntax errors");
        } else {
            details.push("had syntax/runtime errors");
        }

        ScoreResult {
            passed: used_lua_patterns && found_emails && no_errors,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "Did the model use Lua pattern matching (not regex) to extract email addresses? Was the code idiomatic Lua? Did it find both support@example.com and sales@company.org?",
        )
    }
}

// ---------------------------------------------------------------------------
// S8: Data processing (large string global)
// ---------------------------------------------------------------------------

pub struct DataProcessing;

impl Scenario for DataProcessing {
    fn id(&self) -> &str {
        "repl_data_processing"
    }

    fn tags(&self) -> &[&str] {
        &["repl"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        // Generate CSV-like data
        let mut lines = Vec::with_capacity(1001);
        lines.push("name,age,city".to_string());
        let cities = ["New York", "London", "Tokyo", "Paris", "Berlin"];
        for i in 0..1000 {
            let city = cities[i % cities.len()];
            let age = 20 + (i % 50);
            lines.push(format!("person_{},{},{}", i, age, city));
        }
        let csv_data = lines.join("\n");

        repl.with_runtime(|lua| {
            lua.globals().set("csv_data", csv_data.as_str())?;
            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "The variable csv_data contains CSV data with columns: name, age, city. How many people live in Tokyo?"
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let used_repl = !transcript.lua_snippets.is_empty();
        // 1000 entries, Tokyo is every 5th (indices 2, 7, 12, ...) -> 200
        let correct_answer = scoring::final_answer_contains(transcript, "200");

        let mut score = 0.0;
        let mut details = Vec::new();

        if used_repl {
            score += 0.3;
            details.push("used REPL");
        } else {
            details.push("did NOT use REPL");
        }

        if correct_answer {
            score += 0.7;
            details.push("correct answer (200)");
        } else {
            details.push("wrong/missing answer");
        }

        ScoreResult {
            passed: used_repl && correct_answer,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "Did the model process the CSV data using Lua to count people in Tokyo? Was the approach reasonable for processing string data in Lua?",
        )
    }
}
