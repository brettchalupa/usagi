//! Usagi tools window. Placeholder for now; eventually hosts the SFX
//! jukebox and tile picker. Accepts an optional project path which future
//! tools will use to locate sprites.png, sfx/, etc.

use mlua::prelude::*;
use sola_raylib::prelude::*;

pub fn run(project_path: Option<&str>) -> LuaResult<()> {
    let (mut rl, thread) = sola_raylib::init()
        .size(480, 270)
        .title("USAGI TOOLS")
        .highdpi()
        .resizable()
        .build();
    rl.set_target_fps(60);

    let bg = Color::new(20, 20, 30, 255);
    let accent = Color::new(255, 204, 170, 255); // peach
    let muted = Color::new(140, 140, 160, 255);

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(bg);

        d.draw_text("Usagi Tools", 20, 20, 28, accent);
        d.draw_text("hello world", 20, 58, 20, Color::WHITE);

        match project_path {
            Some(p) => d.draw_text(
                &format!("project: {}", p),
                20,
                100,
                16,
                Color::new(180, 180, 200, 255),
            ),
            None => d.draw_text("no project path given", 20, 100, 16, muted),
        }

        d.draw_text("(jukebox + tile picker coming soon)", 20, 140, 14, muted);
    }

    Ok(())
}
