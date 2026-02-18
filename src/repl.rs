use crate::runtime;
use std::sync::mpsc;

pub struct Repl {
    runtime: mlua::Lua,
    output_receiver: mpsc::Receiver<String>,
}

pub struct Eval {
    pub result: Result<Vec<String>, String>,
    pub output: Vec<String>,
}

impl Repl {
    pub fn new() -> Result<Self, mlua::Error> {
        Self::new_with(runtime::default()?)
    }

    pub fn new_with(runtime: mlua::Lua) -> Result<Self, mlua::Error> {
        let output_receiver = runtime::output::capture_output(&runtime)?;

        Ok(Self {
            runtime,
            output_receiver,
        })
    }

    pub async fn eval(&self, code: &str) -> Result<Eval, mlua::Error> {
        let result = match self.runtime.load(code).eval::<mlua::MultiValue>() {
            Ok(values) => Ok(values
                .iter()
                .map(|v| format!("{:#?}", v))
                .collect::<Vec<_>>()),
            Err(e) => Err(Self::format_lua_error(&e)),
        };

        let output = self.output_receiver.try_iter().collect();

        Ok(Eval { result, output })
    }

    fn format_lua_error(error: &mlua::Error) -> String {
        match error {
            mlua::Error::RuntimeError(msg) => format!("RuntimeError: {}", msg),
            mlua::Error::SyntaxError { message, .. } => format!("SyntaxError: {}", message),
            mlua::Error::MemoryError(msg) => format!("MemoryError: {}", msg),
            mlua::Error::CallbackError { traceback, cause } => {
                format!("CallbackError: {}\nTraceback:\n{}", cause, traceback)
            }
            _ => format!("{}", error),
        }
    }
}
