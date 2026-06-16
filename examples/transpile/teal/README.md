# Teal to Lua Usagi Game Dev

Teal is a statically-typed dialect of Lua. Teal is to Lua what TypeScript is to
JavaScript. It allows you to add types for your code and check those types.

Website: [https://teal-language.org/](https://teal-language.org/)

## Developing

1. Install the `tl` binary: `luarocks install tl` or download from
   [https://github.com/teal-language/tl/releases](https://github.com/teal-language/tl/releases)
2. Write your code in `main.tl`, defining the `global` functions needed for
   Usagi's callbacks, like `global function _draw`.
3. Check your types with `tl check main.tl`
4. Transpile your Teal to Lua with: `tl gen main.tl`
5. Run your Usagi game with `usagi dev`

I don't yet know how to set up a file watcher for Teal's compiler, so that'd be
a nice contribution if anyone knows how.

## Defining Usagi's Types

In the `main.tl` example, you'll find an example of how to define the types for
Usagi. If the community finds themselves using Teal, it'd be nice to generate a
complete and shared type definition file for Usagi.

Docs:
[https://teal-language.org/book/latest/declaration_files.html](https://teal-language.org/book/latest/declaration_files.html)
