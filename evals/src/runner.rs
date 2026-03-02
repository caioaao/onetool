use genai::chat::{ChatMessage, ChatRequest, ChatResponse, ToolResponse};
use onetool::genai::LuaRepl;
use std::time::{Duration, Instant};

/// Full record of an agentic interaction.
#[derive(Debug, Clone)]
pub struct Transcript {
    /// Every Lua snippet the model sent as tool calls.
    pub lua_snippets: Vec<String>,
    /// Lua errors encountered (from tool responses containing "error:").
    pub lua_errors: Vec<String>,
    /// Tool response contents (raw JSON strings).
    pub tool_responses: Vec<String>,
    /// The model's final text answer (if any).
    pub final_answer: Option<String>,
    /// Number of LLM round-trips.
    pub total_turns: usize,
    /// Wall-clock duration.
    pub duration: Duration,
}

/// Runs a multi-turn agentic loop, returning the full transcript.
pub async fn run(
    client: &genai::Client,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    lua_repl: &LuaRepl<'_>,
    max_turns: usize,
) -> Result<Transcript, genai::Error> {
    let start = Instant::now();

    let mut messages: Vec<ChatMessage> = vec![
        ChatMessage::system(system_prompt),
        ChatMessage::user(user_prompt),
    ];

    let mut lua_snippets = Vec::new();
    let mut lua_errors = Vec::new();
    let mut tool_responses = Vec::new();
    let mut final_answer = None;
    let mut total_turns = 0;

    for _ in 0..max_turns {
        total_turns += 1;

        let chat_req = ChatRequest::new(messages.clone()).with_tools(vec![lua_repl.definition()]);

        let chat_res: ChatResponse = client.exec_chat(model, chat_req, None).await?;
        let tool_calls = chat_res.clone().into_tool_calls();

        if tool_calls.is_empty() {
            // Final text response
            final_answer = chat_res.first_text().map(|s| s.to_string());
            messages.push(ChatMessage::assistant(
                final_answer.as_deref().unwrap_or(""),
            ));
            break;
        }

        // Process tool calls
        messages.push(tool_calls.clone().into());

        for tool_call in &tool_calls {
            // Extract source code
            if let Some(serde_json::Value::String(code)) = tool_call.fn_arguments.get("source_code")
            {
                lua_snippets.push(code.clone());
            }

            // Execute
            let tool_response: ToolResponse = lua_repl.call(tool_call);
            tool_responses.push(tool_response.content.clone());

            // Check for errors in the response
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&tool_response.content) {
                if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
                    lua_errors.push(err.to_string());
                }
                if let Some(result) = json.get("result").and_then(|r| r.as_str())
                    && result.starts_with("error:")
                {
                    lua_errors.push(result.to_string());
                }
            }

            messages.push(tool_response.into());
        }
    }

    Ok(Transcript {
        lua_snippets,
        lua_errors,
        tool_responses,
        final_answer,
        total_turns,
        duration: start.elapsed(),
    })
}
