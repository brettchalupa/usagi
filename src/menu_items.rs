//! Custom pause-menu items registered from Lua. Games can call
//! `usagi.menu_item("Title Screen", function() ... end)` to add up
//! to `MENU_ITEM_LIMIT` rows between Continue and Settings on the
//! pause menu's Top view.
//!
//! Registration over the cap returns a Lua-side error (surfaced via
//! the engine's usual error overlay) so the dev sees the problem
//! immediately rather than silently losing the call. Items
//! auto-clear before each `_init` re-run (Reset Game / F5) so a
//! script that registers in `_init` doesn't accumulate duplicates.
//! Callbacks fire on BTN1 / Enter selection; the menu closes after
//! the call unless the callback returns Lua `true`.

use mlua::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Maximum number of items a game can register at once. Picked low
/// to keep the Top list scannable and to discourage piling secondary
/// game menus into the pause overlay. Going past the cap is a
/// programmer error, not a silent drop.
pub const MENU_ITEM_LIMIT: usize = 3;

/// One registered entry: the label to draw and the Lua callback to
/// invoke when the player selects it. The callback is stashed in
/// Lua's registry so it survives across the function-call boundary.
pub struct MenuItem {
    pub label: String,
    pub callback: LuaRegistryKey,
}

pub type MenuItemStore = Rc<RefCell<Vec<MenuItem>>>;

pub fn new_store() -> MenuItemStore {
    Rc::new(RefCell::new(Vec::with_capacity(MENU_ITEM_LIMIT)))
}

/// Snapshot the labels for the pause menu's draw / nav code. Cloned
/// once per frame so the menu doesn't have to hold a `RefCell` borrow
/// across its draw call.
pub fn snapshot_labels(store: &MenuItemStore) -> Vec<String> {
    store.borrow().iter().map(|i| i.label.clone()).collect()
}

/// Drains every registered item, removing each callback from Lua's
/// registry. Used by `reset_game` before re-running `_init` so the
/// fresh `_init` starts from an empty slate.
pub fn drain_into_lua(store: &MenuItemStore, lua: &Lua) {
    for item in store.borrow_mut().drain(..) {
        // Registry removal failures only happen for invalid keys; the
        // values came from this same Lua, so this should never fail.
        // Ignore the result so a transient quirk can't poison reset.
        let _ = lua.remove_registry_value(item.callback);
    }
}

/// Installs `usagi.menu_item` and `usagi.clear_menu_items` against the
/// shared store. Both attach to the existing `usagi` Lua table set up
/// by `api::setup_api`.
pub fn register_api(lua: &Lua, store: &MenuItemStore) -> LuaResult<()> {
    let usagi: LuaTable = lua.globals().get("usagi")?;

    let s = Rc::clone(store);
    let menu_item = lua.create_function(move |lua, (label, cb): (String, LuaFunction)| {
        let mut items = s.borrow_mut();
        if items.len() >= MENU_ITEM_LIMIT {
            return Err(LuaError::RuntimeError(format!(
                "usagi.menu_item: cap of {MENU_ITEM_LIMIT} items reached; \
                 call usagi.clear_menu_items() to reset"
            )));
        }
        let callback = lua.create_registry_value(cb)?;
        items.push(MenuItem { label, callback });
        Ok(())
    })?;
    usagi.set(
        "menu_item",
        crate::api::wrap(lua, menu_item, "usagi.menu_item", &["string", "function"])?,
    )?;

    let s = Rc::clone(store);
    let clear = lua.create_function(move |lua, ()| {
        drain_into_lua(&s, lua);
        Ok(())
    })?;
    usagi.set(
        "clear_menu_items",
        crate::api::wrap(lua, clear, "usagi.clear_menu_items", &[])?,
    )?;

    Ok(())
}
