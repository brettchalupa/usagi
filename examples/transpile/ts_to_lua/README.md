# TypeScriptToLua Usagi Example

An example showing how to use TypeScript with Usagi Engine by transpiling it to
Lua. Includes updating, drawing, shape primitive, text, and importing exported
TS functions.

Using https://typescripttolua.github.io/

Requires Node.js to be installed.

Install the package and its CLI:

```
npm install -D typescript typescript-to-lua
```

Compile your TypeScript to Lua:

```
npx tstl
```

## Using with Usagi's Dev Mode

Start the transpiler in watch mode:

```
npm run dev:tstl
```

Start Usagi's dev mode:

```
npm run dev:usagi
```

Export your game:

```
npm run export
```

## Things Worth Knowing

Look at `./main.ts` to see how to structure your code. Usagi relies on globals,
so https://typescripttolua.github.io/docs/assigning-global-variables is followed
by declaring the `_config`, `_update`, and `_draw` functions. It'd be nice if
there were typedefs for Usagi, but for now, it has to be done manually. This
would be a good community contribution. There are a few typedefs to get started.
