// use better_agent::{agent::Agent, llm::GenAIClient, lua::LuaRepl};
use serde_json::{Value, json};
use tracing_subscriber::EnvFilter;

const MODEL: &str = "gpt-4o-mini"; // or "gemini-2.0-flash" or other model supporting tool calls

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber (controlled via RUST_LOG env var)
    // Example: RUST_LOG=debug cargo run --example basic
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Create the Lua REPL (sandboxed environment)
    let repl = onetool::Repl::new().expect("Failed to create REPL");

    let genai_client = genai::Client::default();

    let one_tool = genai::chat::Tool::new("repl")
        .with_description(
            "Run code inside a long-lived Lua REPL. Output and eval result are returned.",
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "source_code": {
                    "type": "string",
                    "description": "The source code to be executed in the Lua REPL runtie"
                }
            },
            "required": ["source_code"]
        }));

    let chat_req = genai::chat::ChatRequest::new(vec![genai::chat::ChatMessage::user(
        "What's the sum of the 10 first prime numbers?",
    )])
    .with_tools(vec![one_tool]);

    println!("--- Getting function call from model");
    let chat_res = genai_client
        .exec_chat(MODEL, chat_req.clone(), None)
        .await
        .unwrap();

    let tool_calls = chat_res.into_tool_calls();

    if tool_calls.is_empty() {
        return Err("Expected tool calls in the response".into());
    }

    println!("--- Tool calls received:");
    for tool_call in &tool_calls {
        println!("Function: {}", tool_call.fn_name);
        println!("Arguments: {}", tool_call.fn_arguments);
    }

    let first_tool_call = &tool_calls[0];
    let source_code = match &first_tool_call.fn_arguments["source_code"] {
        Value::String(source) => Ok(source),
        _ => Err("Expected string but received other"),
    }?;

    let response = repl.eval(&source_code).await?;

    let tool_response = genai::chat::ToolResponse::new(
        first_tool_call.call_id.clone(),
        json!({
        "output": response.output.join("\n"),
            "result": match response.result {
                Ok(result) => result.join("\n"),
                Err(err) => format!("error: {}", err),
            }
            })
        .to_string(),
    );

    let chat_req = chat_req
        .append_message(tool_calls)
        .append_message(tool_response);

    println!("\n--- Getting final response with function results");
    let chat_res = genai_client.exec_chat_stream(MODEL, chat_req, None).await?;

    println!("\n--- Final response:");
    genai::chat::printer::print_chat_stream(chat_res, None).await?;

    Ok(())
}
