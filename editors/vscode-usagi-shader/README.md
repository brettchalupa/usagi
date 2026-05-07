# Usagi Shader VS Code Extension

This extension provides editor support for `.usagi.fs` files by launching the
Usagi CLI language server:

```sh
usagi shaders lsp
```

It contributes the `usagi-shader` language, syntax highlighting, diagnostics,
completions, hover docs, signature help, symbol outline, go-to definition,
generated GLSL preview, and a project shader-check command.

## Development Install

1. Build or install `usagi` so the executable is on `PATH`.
2. Open this folder in VS Code.
3. Press F5 to launch an Extension Development Host.
4. Open a project with `shaders/*.usagi.fs`.

If the executable is not on `PATH`, set `usagi.shader.serverPath` to the full
path of `usagi.exe` or `usagi`.

## Commands

- `Usagi Shader: Select Target Profile`: choose `desktop`, `web`, or `all` for
  live diagnostics.
- `Usagi Shader: Show Generated GLSL`: preview generated GLSL for `desktop`,
  `web`, or staged `glsl440`.
- `Usagi Shader: Check Project Shaders`: run
  `usagi shaders check . --target <target> --format json` in the workspace
  terminal.
- `Usagi Shader: Restart Language Server`: restart the stdio language server.

## Settings

- `usagi.shader.serverPath`: executable path used for `usagi shaders lsp`.
- `usagi.shader.target`: diagnostic target, one of `desktop`, `web`, or `all`.
