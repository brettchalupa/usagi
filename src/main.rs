use mlua::prelude::*;
use sola_raylib::prelude::*;

const GAME_WIDTH: f32 = 320.;
const GAME_HEIGHT: f32 = 180.;

/// draws the game's render target to the screen, scaled
fn draw_render_target(
    d: &mut RaylibDrawHandle,
    rt: &mut RenderTexture2D,
    screen_w: i32,
    screen_h: i32,
    pixel_perfect: bool,
) {
    let game_w = GAME_WIDTH;
    let game_h = GAME_HEIGHT;
    let mut scale = (screen_w as f32 / game_w).min(screen_h as f32 / game_h);
    if pixel_perfect {
        scale = scale.floor();
    }
    if scale < 1.0 {
        scale = 1.0;
    }
    let scaled_w = game_w * scale;
    let scaled_h = game_h * scale;
    let dest_rect = Rectangle {
        x: (screen_w / 2) as f32,
        y: (screen_h / 2) as f32,
        width: scaled_w,
        height: scaled_h,
    };
    let origin = Vector2::new(scaled_w / 2.0, scaled_h / 2.0);

    d.draw_texture_pro(
        rt.texture(),
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: game_w,
            height: -game_h,
        },
        dest_rect,
        origin,
        0.,
        Color::WHITE,
    );
}

/// converts integer color into Color enum
fn palette(c: i32) -> Color {
    match c {
        0 => Color::BLACK,
        7 => Color::WHITE,
        _ => Color::MAGENTA,
    }
}

/// converts the u32 into the `KeyboardKey` enum
fn key_from_u32(k: u32) -> Option<KeyboardKey> {
    use KeyboardKey::*;
    match k {
        x if x == KEY_LEFT as u32 => Some(KEY_LEFT),
        x if x == KEY_RIGHT as u32 => Some(KEY_RIGHT),
        x if x == KEY_UP as u32 => Some(KEY_UP),
        x if x == KEY_DOWN as u32 => Some(KEY_DOWN),
        x if x == KEY_Z as u32 => Some(KEY_Z),
        x if x == KEY_X as u32 => Some(KEY_X),
        _ => None,
    }
}

/// Reads the script file and executes it on the given Lua VM, redefining
/// the `_init` / `_update` / `_draw` globals. Used for both initial load
/// and live reload.
fn load_script(lua: &Lua, path: &str) -> LuaResult<()> {
    let source = std::fs::read_to_string(path).map_err(LuaError::external)?;
    lua.load(&source).set_name(path).exec()
}

/// Install constant tables (`gfx`, `input`, `usagi`) on the Lua globals.
/// Per-frame closures (gfx.clear, input.pressed, ...) are installed inside
/// `lua.scope` blocks since they borrow frame-local Rust state.
fn setup_api(lua: &Lua) -> LuaResult<()> {
    let gfx = lua.create_table()?;
    gfx.set("COLOR_BLACK", 0)?;
    gfx.set("COLOR_WHITE", 7)?;
    lua.globals().set("gfx", gfx)?;

    let input = lua.create_table()?;
    input.set("LEFT", KeyboardKey::KEY_LEFT as u32)?;
    input.set("RIGHT", KeyboardKey::KEY_RIGHT as u32)?;
    input.set("UP", KeyboardKey::KEY_UP as u32)?;
    input.set("DOWN", KeyboardKey::KEY_DOWN as u32)?;
    input.set("A", KeyboardKey::KEY_Z as u32)?;
    input.set("B", KeyboardKey::KEY_X as u32)?;
    lua.globals().set("input", input)?;

    let usagi = lua.create_table()?;
    usagi.set("gfx", lua.globals().get::<LuaTable>("gfx")?)?;
    usagi.set("input", lua.globals().get::<LuaTable>("input")?)?;
    usagi.set("GAME_W", GAME_WIDTH)?;
    usagi.set("GAME_H", GAME_HEIGHT)?;
    lua.globals().set("usagi", usagi)?;

    Ok(())
}

fn main() -> LuaResult<()> {
    let script_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/hello_usagi.lua".to_string());

    let (mut rl, thread) = sola_raylib::init()
        .size((GAME_WIDTH * 2.) as i32, (GAME_HEIGHT * 2.) as i32)
        .highdpi()
        .resizable()
        .title("USAGI")
        .build();
    rl.set_target_fps(60);
    let mut rt: RenderTexture2D = rl
        .load_render_texture(&thread, GAME_WIDTH as u32, GAME_HEIGHT as u32)
        .unwrap();

    let lua = Lua::new();
    setup_api(&lua)?;
    load_script(&lua, &script_path)?;

    if let Ok(init) = lua.globals().get::<LuaFunction>("_init") {
        init.call::<()>(())?;
    }
    let mut update: Option<LuaFunction> = lua.globals().get("_update").ok();
    let mut draw: Option<LuaFunction> = lua.globals().get("_draw").ok();
    let mut last_modified = std::fs::metadata(&script_path)
        .and_then(|m| m.modified())
        .ok();

    while !rl.window_should_close() {
        // Live reload: on mtime change, re-exec the script on the same Lua
        // VM so _update / _draw pick up the new definitions next frame.
        //
        // State is intentionally preserved — we do NOT call _init() here.
        // The point of live reload during prototyping is to tweak logic
        // without losing the current play session (player position, enemy
        // state, etc.). F5 below is the explicit "reset" escape hatch.
        //
        // Caveat for script authors: top-level `local` bindings get fresh
        // nil values on re-exec, so callbacks that captured them will see
        // nil. State that needs to survive reload should live in globals
        // (or use `x = x or <init>` to preserve across re-exec).
        //
        // Reload errors are logged, not fatal — the previous callbacks keep
        // running so a half-saved or syntactically-broken file can't kill
        // the dev session.
        if let Ok(modified) = std::fs::metadata(&script_path).and_then(|m| m.modified())
            && Some(modified) != last_modified
        {
            last_modified = Some(modified);
            match load_script(&lua, &script_path) {
                Ok(()) => {
                    println!("[usagi] reloaded {}", script_path);
                    update = lua.globals().get("_update").ok();
                    draw = lua.globals().get("_draw").ok();
                }
                Err(e) => eprintln!("[usagi] reload failed: {}", e),
            }
        }

        // Dev shortcut: F5 runs _init() to wipe game state. Paired with the
        // preserve-state live-reload above; this is the one way to actually
        // reset during a session without restarting the process.
        if rl.is_key_pressed(KeyboardKey::KEY_F5)
            && let Ok(init) = lua.globals().get::<LuaFunction>("_init")
        {
            match init.call::<()>(()) {
                Ok(()) => println!("[usagi] reset (F5)"),
                Err(e) => eprintln!("[usagi] _init error: {}", e),
            }
        }

        let dt = rl.get_frame_time();
        let screen_w = rl.get_screen_width();
        let screen_h = rl.get_screen_height();
        let fps = rl.get_fps();

        // Update phase: input.pressed borrows rl (immutable — is_key_down is &self)
        if let Some(ref update_fn) = update {
            let input_tbl: LuaTable = lua.globals().get("input")?;
            let rl_ref = &rl;
            lua.scope(|scope| {
                let pressed = scope.create_function(|_, key: u32| {
                    Ok(key_from_u32(key).is_some_and(|k| rl_ref.is_key_pressed(k)))
                })?;
                input_tbl.set("pressed", pressed)?;
                let down = scope.create_function(|_, key: u32| {
                    Ok(key_from_u32(key).is_some_and(|k| rl_ref.is_key_down(k)))
                })?;
                input_tbl.set("down", down)?;
                update_fn.call::<()>(dt)?;
                Ok(())
            })?;
        }

        // Draw phase: gfx.* share d_rt via RefCell (multiple draw fns need mut access)
        {
            let mut d_rt = rl.begin_texture_mode(&thread, &mut rt);
            if let Some(ref draw_fn) = draw {
                let gfx_tbl: LuaTable = lua.globals().get("gfx")?;
                let d_rt_cell = std::cell::RefCell::new(&mut d_rt);
                lua.scope(|scope| {
                    let clear = scope.create_function(|_, c: i32| {
                        d_rt_cell.borrow_mut().clear_background(palette(c));
                        Ok(())
                    })?;
                    let text =
                        scope.create_function(|_, (s, x, y, c): (String, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_text(
                                &s,
                                x.round() as i32,
                                y.round() as i32,
                                8,
                                palette(c),
                            );
                            Ok(())
                        })?;
                    let rect = scope.create_function(
                        |_, (x, y, w, h, c): (f32, f32, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_rectangle(
                                x.round() as i32,
                                y.round() as i32,
                                w.round() as i32,
                                h.round() as i32,
                                palette(c),
                            );
                            Ok(())
                        },
                    )?;
                    gfx_tbl.set("clear", clear)?;
                    gfx_tbl.set("text", text)?;
                    gfx_tbl.set("rect", rect)?;
                    draw_fn.call::<()>(dt)?;
                    Ok(())
                })?;
            }
            d_rt.draw_text(&format!("FPS: {}", fps), 0, 0, 8, Color::GREEN);
        }

        // Blit render target to screen
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::BLACK);
            draw_render_target(&mut d, &mut rt, screen_w, screen_h, true);
        }
    }
    Ok(())
}
