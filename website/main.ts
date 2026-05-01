import { CSS, render } from "@deno/gfm";
import { serveFile } from "@std/http/file-server";
import "prism-lua";

const NOT_FOUND_MD = `# Not Found

The page you are looking for could not be found.

[Return home.](/)
`;

function renderPage(title: string, body: string): string {
  return `<!DOCTYPE html>
    <html lang="en">
      <head>
        <title>${title}</title>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <meta property="og:title" content="Usagi Engine" />
        <meta property="og:type" content="website" />
        <meta property="og:url" content="https://usagiengine.com/" />
        <meta property="og:image" content="https://usagiengine.com/og.png" />
        <meta property="og:image:type" content="image/png" />
        <meta property="og:image:alt" content="Usagi Logo: pixel art bunny, Usagi Engine - Rapid 2D Prototyping" />
        <link rel="icon" href="favicon.png" />
        <meta
          name="description"
          content="Usagi is a free and open source game engine for making pixel art games coded with Lua. It features live reloading of code and assets during development and cross-platform export in a single command."
          />
        <style>
        body {
          max-width: 800px;
          margin: 24px auto;
          padding: 12px;
        }
        ${CSS}
        </style>
      </head>
      <body
        data-color-mode="auto"
        data-light-theme="light"
        data-dark-theme="dark"
        class="markdown-body"
      >
        ${body}
      </body>
    </html>
  `;
}

async function handler(req: Request): Promise<Response> {
  const url = new URL(req.url);
  console.log(`[${req.method}]`, url.pathname);
  try {
    if (url.pathname == "/" || url.pathname == "index.html") {
      const markdown = await Deno.readTextFile("../README.md");
      const body = render(markdown);
      const html = renderPage("Usagi Engine - Rapid 2D Game Prototyping", body);
      return new Response(html, {
        headers: {
          "content-type": "text/html;charset=utf-8",
        },
      });
    }
    if (url.pathname == "/favicon.png") {
      return serveFile(req, "./favicon.png");
    }
    if (url.pathname == "/website/card-logo.png") {
      return serveFile(req, "./card-logo.png");
    }
    if (url.pathname == "/og.png") {
      return serveFile(req, "./og.png");
    }

    const html = renderPage(
      "Not Found | Usagi Engine",
      render(NOT_FOUND_MD),
    );
    return new Response(html, {
      headers: {
        "content-type": "text/html;charset=utf-8",
      },
    });
  } catch (err) {
    console.error(err);
    return new Response((err as Error).message, { status: 500 });
  }
}
const port = Deno.env.get("PORT") || "8008";
Deno.serve({ port: parseInt(port) }, handler);
