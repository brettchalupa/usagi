/************** Bare minimum type declarations for this example *************/

declare var _config: (this: void) => Record<string, string | number | boolean>;
declare var _update: (this: void, dt: number) => void;
declare var _draw: (this: void, dt: number) => void;

/** @noSelf **/
declare interface InputInterface {
  RIGHT: number;
  LEFT: number;
  held: (btn: number) => boolean;
}
/** @noSelf **/
declare interface GfxInterface {
  rect_fill: (
    x: number,
    y: number,
    w: number,
    h: number,
    color: number,
  ) => void;
  text: (
    text: string,
    x: number,
    y: number,
    color: number,
  ) => void;
  COLOR_WHITE: number;
  COLOR_BLACK: number;
  clear: (color: number) => void;
}
declare const input: InputInterface;
declare const gfx: GfxInterface;

import greet from "./greet";

let state = {
  x: 10,
  y: 40,
};
_config = () => {
  return { game_width: 240, game_height: 240, name: "TS to Lua" };
};

_update = (dt: number) => {
  // px/sec
  let spd = 100;
  if (input.held(input.LEFT)) {
    state.x -= spd * dt;
  }

  if (input.held(input.RIGHT)) {
    state.x += spd * dt;
  }
};

_draw = (dt: number) => {
  gfx.clear(gfx.COLOR_BLACK);
  gfx.rect_fill(state.x, state.y, 16, 16, gfx.COLOR_WHITE);

  gfx.text(greet("Alucard"), 4, 4, gfx.COLOR_WHITE);
};
