use crate::runner::Transcript;
use genai::chat::{ChatMessage, ChatRequest};
use serde::Serialize;

/// Result of scoring a scenario.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreResult {
    pub passed: bool,
    pub score: f64,
    pub details: String,
}

/// Result from LLM-as-judge evaluation.
#[derive(Debug, Clone, Serialize)]
pub struct JudgeResult {
    pub score: f64,
    pub rationale: String,
}

// ---------------------------------------------------------------------------
// Deterministic scoring helpers
// ---------------------------------------------------------------------------

/// Returns true if any Lua snippet contains the given substring.
pub fn transcript_contains_lua(transcript: &Transcript, pattern: &str) -> bool {
    transcript.lua_snippets.iter().any(|s| s.contains(pattern))
}

/// Returns true if any Lua snippet matches a case-insensitive substring.
pub fn transcript_contains_lua_ci(transcript: &Transcript, pattern: &str) -> bool {
    let lower = pattern.to_lowercase();
    transcript
        .lua_snippets
        .iter()
        .any(|s| s.to_lowercase().contains(&lower))
}

/// Returns true if the final answer contains the given substring.
pub fn final_answer_contains(transcript: &Transcript, text: &str) -> bool {
    transcript
        .final_answer
        .as_ref()
        .is_some_and(|a| a.contains(text))
}

/// Returns true if the final answer contains the given substring (case-insensitive).
pub fn final_answer_contains_ci(transcript: &Transcript, text: &str) -> bool {
    let lower = text.to_lowercase();
    transcript
        .final_answer
        .as_ref()
        .is_some_and(|a| a.to_lowercase().contains(&lower))
}

/// Returns true if any tool response contains the given substring.
pub fn tool_response_contains(transcript: &Transcript, text: &str) -> bool {
    transcript.tool_responses.iter().any(|r| r.contains(text))
}

/// Returns true if the model queried the `docs` global.
pub fn docs_queried(transcript: &Transcript) -> bool {
    transcript_contains_lua(transcript, "docs")
}

/// Returns true if the transcript has no Lua errors.
pub fn no_lua_errors(transcript: &Transcript) -> bool {
    transcript.lua_errors.is_empty()
}

// ---------------------------------------------------------------------------
// LLM-as-Judge
// ---------------------------------------------------------------------------

const JUDGE_SYSTEM: &str = r#"You are an evaluation judge. You will receive a transcript of an LLM interacting with a Lua REPL tool, along with a scoring prompt.

Score the interaction on a 0.0 to 1.0 scale. Respond with ONLY a JSON object:
{"score": <float>, "rationale": "<brief explanation>"}

Do not include any text outside the JSON object."#;

/// Sends the transcript + judge prompt to an LLM and parses the 0-1 score.
pub async fn judge(
    client: &genai::Client,
    judge_model: &str,
    transcript: &Transcript,
    judge_prompt: &str,
) -> Result<JudgeResult, genai::Error> {
    let transcript_text = format_transcript_for_judge(transcript);

    let user_message = format!(
        "## Transcript\n\n{}\n\n## Scoring Criteria\n\n{}",
        transcript_text, judge_prompt
    );

    let chat_req = ChatRequest::new(vec![
        ChatMessage::system(JUDGE_SYSTEM),
        ChatMessage::user(user_message),
    ]);

    let chat_res = client.exec_chat(judge_model, chat_req, None).await?;
    let text = chat_res.first_text().unwrap_or("");

    // Parse JSON from response (tolerate surrounding whitespace/text)
    parse_judge_response(text)
}

fn format_transcript_for_judge(transcript: &Transcript) -> String {
    let mut parts = Vec::new();

    for (i, snippet) in transcript.lua_snippets.iter().enumerate() {
        parts.push(format!("### Tool Call {}\n```lua\n{}\n```", i + 1, snippet));

        if let Some(response) = transcript.tool_responses.get(i) {
            parts.push(format!(
                "### Tool Response {}\n```json\n{}\n```",
                i + 1,
                response
            ));
        }
    }

    if !transcript.lua_errors.is_empty() {
        parts.push(format!(
            "### Errors\n{}",
            transcript
                .lua_errors
                .iter()
                .map(|e| format!("- {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if let Some(answer) = &transcript.final_answer {
        parts.push(format!("### Final Answer\n{}", answer));
    }

    parts.push(format!("Total turns: {}", transcript.total_turns));

    parts.join("\n\n")
}

fn parse_judge_response(text: &str) -> Result<JudgeResult, genai::Error> {
    // Try to find JSON in the response
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    #[derive(serde::Deserialize)]
    struct JudgeJson {
        score: f64,
        rationale: String,
    }

    match serde_json::from_str::<JudgeJson>(json_str) {
        Ok(parsed) => Ok(JudgeResult {
            score: parsed.score.clamp(0.0, 1.0),
            rationale: parsed.rationale,
        }),
        Err(_) => {
            // Fallback: couldn't parse judge response
            Ok(JudgeResult {
                score: 0.0,
                rationale: format!("Failed to parse judge response: {}", text),
            })
        }
    }
}
