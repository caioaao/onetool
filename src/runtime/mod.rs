pub mod docs;
pub mod output;
pub mod sandbox;

pub fn default() -> mlua::Lua {
    let lua = mlua::Lua::new();
    sandbox::apply(&lua).expect("Failed to apply sandbox");
    lua
}
