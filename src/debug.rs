use std::{cell::RefCell, collections::HashMap, rc::Rc};

use sola_raylib::{
    RaylibHandle,
    color::Color,
    drawing::{RaylibDraw, RaylibTextureMode},
    math::Vector2,
    text::Font,
};

pub struct Debug {
    msgs: HashMap<String, String>,
    visible: bool,
    pub fps: u32,
    dev: bool,
}

impl Debug {
    pub fn new(dev: bool) -> Self {
        Debug {
            msgs: HashMap::new(),
            visible: false,
            fps: 0,
            dev,
        }
    }

    pub fn draw(&self, rt: &mut RaylibTextureMode<'_, RaylibHandle>, font: &Font) {
        if self.visible {
            rt.draw_text_ex(
                font,
                &format!("FPS: {}", self.fps),
                Vector2::new(1.0, 1.0),
                crate::font::MONOGRAM_SIZE as f32,
                0.0,
                Color::GREEN,
            );

            if self.dev {
                self.msgs.iter().enumerate().for_each(|(i, (k, v))| {
                    rt.draw_text_ex(
                        font,
                        &format!("{k}: {v}"),
                        Vector2::new(
                            1.0,
                            1.0 + (i as f32 + 1.0) * crate::font::MONOGRAM_SIZE as f32,
                        ),
                        crate::font::MONOGRAM_SIZE as f32,
                        0.0,
                        Color::GREEN,
                    );
                });
            }
        }
    }

    pub fn tick(&mut self, fps: u32) {
        self.fps = fps;
    }

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }
}

pub fn register_api(lua: &mlua::Lua, state: &Rc<RefCell<Debug>>) -> mlua::Result<()> {
    let usagi: mlua::Table = lua.globals().get("usagi")?;

    let table = lua.create_table()?;

    let debug_set = state.clone();
    table.set(
        "set",
        lua.create_function(move |_, (key, value): (String, String)| {
            debug_set.borrow_mut().msgs.insert(key, value);
            Ok(())
        })?,
    )?;

    let debug_get = state.clone();
    table.set(
        "get",
        lua.create_function(move |_, key: String| Ok(debug_get.borrow().msgs.get(&key).cloned()))?,
    )?;

    usagi.set("debug", table)?;

    Ok(())
}
