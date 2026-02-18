pub mod docs;
pub mod output;
pub mod sandbox;

pub fn default() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    sandbox::apply(&lua)?;

    Ok(lua)
}
