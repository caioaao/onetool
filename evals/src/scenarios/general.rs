use super::Scenario;
use crate::runner::Transcript;
use crate::scoring::{self, ScoreResult};
use onetool::Repl;

// ---------------------------------------------------------------------------
// S13: Needle in haystack
// ---------------------------------------------------------------------------

const MAGIC_NUMBER: u32 = 42857;
const NUM_LINES: usize = 100_000;

pub struct NeedleInHaystack;

impl Scenario for NeedleInHaystack {
    fn id(&self) -> &str {
        "general_needle_in_haystack"
    }

    fn tags(&self) -> &[&str] {
        &["general"]
    }

    fn setup(&self, repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        let mut lines = Vec::with_capacity(NUM_LINES);
        for i in 0..NUM_LINES {
            if i == NUM_LINES / 2 {
                lines.push(format!("The magic number is {}", MAGIC_NUMBER));
            } else {
                lines.push(format!("Line {} contains random filler text data", i));
            }
        }
        let context = lines.join("\n");

        repl.with_runtime(|lua| {
            lua.globals().set("context", context.as_str())?;
            Ok(())
        })?;
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "There is a variable called 'context' that contains a large text. Find the magic number hidden in it."
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let used_repl = !transcript.lua_snippets.is_empty();
        let correct = scoring::final_answer_contains(transcript, &MAGIC_NUMBER.to_string());
        let searched_context = scoring::transcript_contains_lua(transcript, "context");

        let mut score = 0.0;
        let mut details = Vec::new();

        if used_repl && searched_context {
            score += 0.3;
            details.push("searched context via REPL");
        } else if used_repl {
            score += 0.1;
            details.push("used REPL but unclear search");
        } else {
            details.push("did NOT use REPL");
        }

        if correct {
            score += 0.7;
            details.push("found magic number");
        } else {
            details.push("did NOT find magic number");
        }

        ScoreResult {
            passed: used_repl && correct,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "The context variable contains 100k lines with a magic number hidden at the midpoint. Did the model use Lua string search to find it efficiently?",
        )
    }
}

// ---------------------------------------------------------------------------
// S14: Twin primes under 1000
// ---------------------------------------------------------------------------

pub struct TwinPrimes;

impl Scenario for TwinPrimes {
    fn id(&self) -> &str {
        "general_twin_primes"
    }

    fn tags(&self) -> &[&str] {
        &["general"]
    }

    fn setup(&self, _repl: &Repl) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn user_prompt(&self) -> &str {
        "Find all twin prime pairs (p, p+2) where both p and p+2 are prime and less than 1000. How many pairs are there?"
    }

    fn score(&self, transcript: &Transcript) -> ScoreResult {
        let used_repl = !transcript.lua_snippets.is_empty();
        // There are 35 twin prime pairs under 1000
        let correct_count = scoring::final_answer_contains(transcript, "35");
        let no_errors = scoring::no_lua_errors(transcript);

        // Check for some known twin prime pairs
        let has_examples = scoring::final_answer_contains(transcript, "3, 5")
            || scoring::final_answer_contains(transcript, "(3, 5)")
            || scoring::final_answer_contains(transcript, "3,5")
            || scoring::final_answer_contains(transcript, "11, 13")
            || scoring::final_answer_contains(transcript, "(11, 13)");

        let mut score = 0.0;
        let mut details = Vec::new();

        if used_repl {
            score += 0.2;
            details.push("used REPL");
        }

        if correct_count {
            score += 0.5;
            details.push("correct count (35)");
        } else {
            details.push("wrong/missing count");
        }

        if no_errors {
            score += 0.2;
            details.push("valid Lua");
        } else {
            details.push("had Lua errors");
        }

        if has_examples {
            score += 0.1;
            details.push("listed example pairs");
        }

        ScoreResult {
            passed: used_repl && correct_count && no_errors,
            score,
            details: details.join(", "),
        }
    }

    fn judge_prompt(&self) -> Option<&str> {
        Some(
            "Did the model correctly compute all 35 twin prime pairs under 1000 using valid Lua code? Was the algorithm reasonable?",
        )
    }
}
