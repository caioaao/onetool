//! Runs a Lua repl
//! This simulates what an LLM would see when interacting with the runtime

use rustyline::DefaultEditor;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let repl = onetool::Repl::new().expect("Failed to initialize REPL");
    let mut editor = DefaultEditor::new().expect("Failed to create editor");

    println!("Lua REPL");
    println!("---------------------------");
    println!("To understand what the LLM would 'see', experiment with:");
    println!("  - Printing data using `print`");
    println!("  - Seeing the result of plain expressions (`1+1`, `string.gsub(...)`)");
    println!("  - Throwing errors (incomplete inputs, missing fns)");
    println!("Press Ctrl+C or Ctrl+D to exit");
    println!();

    loop {
        loop {
            match editor.readline("> ") {
                Ok(line) => {
                    editor.add_history_entry(&line).unwrap();
                    match repl.eval(&line).await {
                        Ok(result) => {
                            println!("-- OUTPUT:\n{}", result.output.join("\n"));
                            match result.result {
                                Ok(vals) => println!("-- RESULT:\n{}", vals.join("\n")),
                                Err(err) => println!("-- ERROR:\n{}", err),
                            }
                        }
                        Err(err) => {
                            eprintln!("error: {}", err);
                            break;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }
}
