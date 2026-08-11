# Fennel to Lua for Usagi Game Dev

> Fennel is a programming language that brings together the simplicity, speed, and reach of Lua with the flexibility of a [lisp syntax and macro system](https://en.wikipedia.org/wiki/Lisp_\(programming_language\)).

Website: [https://fennel-lang.org/](https://fennel-lang.org/)

## Developing

Here is how to set up a Fennel project with the "ahead-of-time (AOT) compilation" method:

1. Download the fennel *executable* with your [method of choice](https://fennel-lang.org/setup#downloading-fennel) (luarocks, system package manager, direct file download)
2. Write your code in `main.fnl`
3. Assuming the executable is part of the shell's PATH variable, run `fennel --compile main.fnl > main.lua` to produce `main.lua`
  - Call `{dir}/fennel` (full path to the executable) if the PATH variable is not set up
  - It is `main.lua` (not `main.fnl`) that is bundled into the usagi build
4. Start up the Usagi dev mode: `usagi dev`

Game config goes in a `usagi.conf` file next to `main.lua`.

## A Note on Global Assignments

A defined `fn` in any scope (file scope included) is created as a `local function ...`. To assign global callbacks (`_draw`, `_update`, `_init`) for Usagi, use *direct global table assignment* via `(set ...)`.

```fennel
; Create a local function, then assign to _G.
(fn _draw [dt] ...)
(set _G._draw _draw)

; You can also create-and-assign the function to _G directly, as demonstrated below.
(set _G._draw (fn [dt] ...))
```

## Managing multiple source files

The `fennel` executable does not have typical quality-of-like features like direct file output, multiple file arguments, or file watching (for auto-compilation on modifications).

The `Makefile` in this folder demonstrates one cross-platform method of managing a project with *multiple .fnl files*. As an example, if you have `main.fnl` and `text_color.fnl` in the current folder, you can use GNU `make` to execute the instructions from `Makefile`, which runs the compilation command for both files.

```sh
# In the same folder as Makefile, main.fnl, text_color.fnl
$ make
fennel --compile main.fnl > main.lua
fennel --compile text_color.fnl > text_color.lua
```

## FAQ

### Can I embed the Fennel script instead of AOT-compiling the lua source?

Unfortunately, *no*. `usagi export` cannot work, even if `usagi dev` works by coincidence. The reason is due to the implementation of Usagi itself.

Usagi makes Lua's top-level `require` and its own Lua API (`usagi.read_text`, `usagi.read_json`, etc.) perform file accesses through a *virtual file system*. This permits both `usagi dev` (folder setup) and `usagi export` (bundled setup) to work with the same file "layout". Fennel's reliance on its own `require` is incompatible with Usagi, and causes failures during `usagi export`.

```lua
-- Note: Usagi only bundles generic files in the "data" folder.

-- Still won't work. dofile is fennel's "dofile", which searches the real file system and fails to find the bundled file.
require("fennel-1_6_1").install().dofile("data/main.fnl")
-- Still won't work. The second require is fennel's "require". File search fails for the same reason as above.
require("fennel-1_6_1").install().eval("(require :data.main)")
```

If you really want to make this work, a setup is *technically possible* by working around Usagi's overridden `require` logic.

1. Download the fennel *script* with your [method of choice](https://fennel-lang.org/setup#downloading-fennel) (luarocks, system package manager, direct file download)
2. Write your code in `data/main.fnl` (has to reside in `data/` to be read via `usagi.read_text`) 
3. Setup a minimal `main.lua` that loads the Fennel script and interprets the contents of `data/main.fnl`

```lua
-- main.lua
-- Load Fennel source from data/main.fnl
local main_fnl = usagi.read_text("main.fnl")
-- Require the Fennel script, and evaluate the Fennel source code.
require("fennel-1_6_1").eval(main_fnl)
```

4. Start up the Usagi dev mode: `usagi dev`

I suspect a multi-file setup via exact `eval` order is unmaintainable.

### AOT main.lua failure in Windows Powershell < 6.0.0 ?

Trying the AOT compilation flow with Windows Powershell prior to 6.0.0 leads to an improperly-formatted `main.lua` (UTF8 byte-order mark) and a very cryptic error from Usagi:

```
> $PSVersionTable

Name                           Value
----                           -----
PSVersion                      5.1.26100.8875
...

> usagi dev
[usagi] usagi v1.2.0
[usagi] initial load: syntax error: [string ".\main.lua"]:1: unexpected symbol near '<\239>'
```

[Upgrading Window Powershell](https://learn.microsoft.com/en-us/powershell/scripting/install/install-powershell-on-windows) avoids UTF8 byte-order marks, and properly pipes compiled source to `main.lua`.

```
> $PSVersionTable

Name                           Value
----                           -----
PSVersion                      7.6.4
...
```
