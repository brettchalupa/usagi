import { CSS, render } from "@deno/gfm";
import { serveFile } from "@std/http/file-server";
import "prism-lua";

const NOT_FOUND_MD = `# Not Found

The page you are looking for could not be found.

[Return home.](/)
`;

const SITE_URL = "https://usagiengine.com";
const HOME_TITLE = "Usagi Engine - Rapid 2D Game Prototyping";
const DEFAULT_DESCRIPTION =
  "Usagi is a free and open source game engine for making pixel art games coded with Lua. It features live reloading of code and assets during development and cross-platform export in a single command.";

type PageOpts = {
  // Page name; appended with " | Usagi Engine". Omit for the home page.
  title?: string;
  description: string;
  path: string;
  body: string;
};

async function renderPage(opts: PageOpts): Promise<string> {
  const layout = await Deno.readTextFile("./layout.html");
  const fullTitle = opts.title ? `${opts.title} | Usagi Engine` : HOME_TITLE;
  const values: Record<string, string> = {
    "title": fullTitle,
    "description": opts.description,
    "url": `${SITE_URL}${opts.path}`,
    "gfm-css": CSS,
    "body": opts.body,
  };
  return layout.replace(
    /\{\{(title|description|url|gfm-css|body)\}\}/g,
    (_, key) => values[key],
  );
}

async function handler(req: Request): Promise<Response> {
  const url = new URL(req.url);
  console.log(`[${req.method}]`, url.pathname);
  try {
    if (url.pathname == "/" || url.pathname == "index.html") {
      const markdown = await Deno.readTextFile("../README.md");
      const body = render(markdown);
      const html = await renderPage({
        description: DEFAULT_DESCRIPTION,
        path: "/",
        body,
      });
      return new Response(html, {
        headers: {
          "content-type": "text/html;charset=utf-8",
        },
      });
    }
    if (url.pathname.toLowerCase().replace(/\/$/, "") === "/discord") {
      return Response.redirect("https://discord.gg/a92ZjE4NUx", 302);
    }
    if (url.pathname.toLowerCase().replace(/\/$/, "") === "/changelog") {
      const markdown = await Deno.readTextFile("../CHANGELOG.md");
      const body = render(markdown);
      const html = await renderPage({
        title: "Changelog",
        description:
          "Release notes and version history for Usagi Engine, the open source 2D game engine for prototyping with Lua.",
        path: "/changelog",
        body,
      });
      return new Response(html, {
        headers: {
          "content-type": "text/html;charset=utf-8",
        },
      });
    }
    if (url.pathname === "/UNLICENSE" || url.pathname === "/unlicense/") {
      return Response.redirect(new URL("/unlicense", url), 301);
    }
    if (url.pathname.toLowerCase().replace(/\/$/, "") === "/license") {
      return Response.redirect(new URL("/unlicense", url), 301);
    }
    if (url.pathname === "/unlicense") {
      const license = await Deno.readTextFile("../UNLICENSE");
      const body = render(`# License\n\n${license}`);
      const html = await renderPage({
        title: "License",
        description:
          "Usagi Engine is public domain software released under The Unlicense. Free to copy, modify, and use for any purpose.",
        path: "/unlicense",
        body,
      });
      return new Response(html, {
        headers: {
          "content-type": "text/html;charset=utf-8",
        },
      });
    }
    if (
      url.pathname === "/THIRD_PARTY_LICENSES.md" ||
      url.pathname === "/third-parties/"
    ) {
      return Response.redirect(new URL("/third-parties", url), 301);
    }
    if (url.pathname === "/third-parties") {
      const markdown = await Deno.readTextFile("../THIRD_PARTY_LICENSES.md");
      const body = render(markdown);
      const html = await renderPage({
        title: "Third-Party Licenses",
        description:
          "Licenses of every Rust crate Usagi Engine depends on, with full license text. Generated from Cargo.lock by cargo-about.",
        path: "/third-parties",
        body,
      });
      return new Response(html, {
        headers: {
          "content-type": "text/html;charset=utf-8",
        },
      });
    }
    if (url.pathname === "/favicon.png") {
      return serveFile(req, "./favicon.png");
    }
    if (url.pathname === "/install.sh" || url.pathname === "/install.ps1") {
      const file = url.pathname.slice(1);
      const body = await Deno.readTextFile(`./${file}`);
      return new Response(body, {
        headers: {
          // text/plain so browsers show the script instead of saving/running it;
          // curl and irm don't care about content-type.
          "content-type": "text/plain; charset=utf-8",
          "cache-control": "public, max-age=300",
        },
      });
    }
    if (url.pathname === "/website/card-logo.png") {
      return serveFile(req, "./card-logo.png");
    }
    if (url.pathname === "/og.png") {
      return serveFile(req, "./og.png");
    }

    const html = await renderPage({
      title: "Not Found",
      description: "The page you are looking for could not be found.",
      path: url.pathname,
      body: render(NOT_FOUND_MD),
    });
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
