use anyhow::Result;
use mistralrs::{IsqType, RequestBuilder, TextMessageRole, TextModelBuilder, ToolChoice};
use tracing_subscriber::EnvFilter;

const MODEL: &str = "microsoft/Phi-3.5-mini-instruct";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Create the Lua REPL (sandboxed environment)
    let repl = onetool::Repl::new().map_err(|e| anyhow::anyhow!("{}", e))?;

    // Create the tool wrapper for mistralrs
    let lua_repl = onetool::mistralrs::LuaRepl::new(repl);

    // Build the mistralrs model
    let model = TextModelBuilder::new(MODEL)
        .with_logging()
        .with_isq(IsqType::Q8_0)
        .build()
        .await?;

    // Build initial request with tool
    let mut messages = RequestBuilder::new()
        .add_message(
            TextMessageRole::User,
            "What's the sum of the 10 first prime numbers?",
        )
        .set_tools(vec![lua_repl.definition()])
        .set_tool_choice(ToolChoice::Auto);

    println!("--- Getting function call from model");
    let response = model.send_chat_request(messages.clone()).await?;

    let message = &response.choices[0].message;

    if let Some(tool_calls) = &message.tool_calls {
        if tool_calls.is_empty() {
            return Err(anyhow::anyhow!("Expected tool calls in the response"));
        }

        println!("--- Tool calls received:");
        for tool_call in tool_calls {
            println!("Function: {}", tool_call.function.name);
            println!("Arguments: {}", tool_call.function.arguments);
        }

        // Execute the tool call
        let called = &tool_calls[0];
        let result = lua_repl.call(called);

        println!("\n--- Tool result:");
        println!("{}", result);

        // Add tool call and result to conversation
        messages = messages
            .add_message_with_tool_call(
                TextMessageRole::Assistant,
                String::new(),
                vec![called.clone()],
            )
            .add_tool_message(result, called.id.clone())
            .set_tool_choice(ToolChoice::None);

        println!("\n--- Getting final response with function results");
        let response = model.send_chat_request(messages.clone()).await?;

        let message = &response.choices[0].message;
        println!("\n--- Final response:");
        println!("{:?}", message.content);
    }

    Ok(())
}
