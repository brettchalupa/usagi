import { CSS, render } from "@deno/gfm";
import { serveDir } from "@std/http/file-server";
import "prism-lua";
import * as path from "@std/path";

const SITE_URL = "https://usagiengine.com";

type Page = {
  /** Page name; appended with " | Usagi Engine" except for home page */
  title: string;
  /** Meta description for the page inserted into the layout */
  description: string;
  /** Path the page serves at, e.g., `/changelog` */
  path: string;
  /** Relative path to source markdown file to generate as HTML */
  source: string;
};

type Asset = {
  /** Relative path to the file on disk */
  source: string;
  /** Destination in the static site of where the file should live */
  dest: string;
};

const BUILD_DIR = "_build";

/**
 * Renders the passed in page as HTML in the `./layout.html`,
 * subtiting dynamic values, and returns the HTML.
 */
async function renderPage(page: Page): Promise<string> {
  const layout = await Deno.readTextFile("./layout.html");
  let title = page.title;
  if (page.path !== "/") {
    title = `${title} | Usagi Engine`;
  }
  const markdown = await Deno.readTextFile(page.source);
  const body = render(markdown);

  const values: Record<string, string> = {
    "title": title,
    "description": page.description,
    "url": `${SITE_URL}${page.path}`,
    "gfm-css": CSS,
    "body": body,
  };
  return layout.replace(
    /\{\{\s*(title|description|url|gfm-css|body)\s*\}\}/g,
    (_, key) => values[key],
  );
}

// NOTE: these are only for local development testing; in the deployed
// site they are managed via Bunny.net CDN Edge Rules: https://dash.bunny.net/cdn/6679857/edge-rules
const REDIRECTS: Record<string, { dest: string; status: 302 | 301 }> = {
  "/discord": {
    "dest": "https://discord.gg/rmKAx3d3Ww",
    "status": 302,
  },
  "/license": {
    "dest": "/unlicense",
    "status": 301,
  },
  "/UNLICENSE": {
    "dest": "/unlicense",
    "status": 301,
  },
  "/THIRD_PARTY_LICENSES.md": {
    "dest": "/third-parties",
    "status": 301,
  },
};

const PAGES: Page[] = [
  {
    title: "Usagi Engine - Rapid 2D Game Prototyping",
    description:
      "Usagi is a free and open source game engine for making pixel art games coded with Lua. It features live reloading of code and assets during development and cross-platform export in a single command.",
    path: "/index.html",
    source: "../README.md",
  },
  {
    title: "Changelog",
    description:
      "Release notes and version history for Usagi Engine, the open source 2D game engine for prototyping with Lua.",
    path: "/changelog/index.html",
    source: "../CHANGELOG.md",
  },
  {
    title: "License",
    description:
      "Usagi Engine is public domain software released under The Unlicense. Free to copy, modify, and use for any purpose.",
    path: "/unlicense/index.html",
    source: "../UNLICENSE",
  },
  {
    title: "Third-Party Licenses",
    description:
      "Licenses of every Rust crate Usagi Engine depends on, with full license text. Generated from Cargo.lock by cargo-about.",
    path: "/third-parties/index.html",
    source: "../THIRD_PARTY_LICENSES.md",
  },
  {
    title: "Not Found",
    description: "The page you are looking for could not be found.",
    path: "/404.html",
    source: "404.md",
  },
];

const ASSETS: Asset[] = [
  {
    "source": "./favicon.png",
    "dest": "./favicon.png",
  },
  {
    "source": "./install.sh",
    "dest": "./install.sh",
  },
  {
    "source": "./install.ps1",
    "dest": "./install.ps1",
  },
  {
    "source": "./install.ps1",
    "dest": "./install.ps1",
  },
  {
    "source": "./card-logo.png",
    "dest": "./website/card-logo.png",
  },
  {
    "source": "./demo.gif",
    "dest": "./website/demo.gif",
  },
  {
    "source": "./menu.png",
    "dest": "./website/menu.png",
  },
  {
    "source": "./tools.png",
    "dest": "./website/tools.png",
  },

  {
    "source": "./og.png",
    "dest": "./og.png",
  },
];

async function copyAsset(asset: Asset) {
  const filePath = path.join(BUILD_DIR, asset.dest);
  const fileDir = path.dirname(filePath);
  await Deno.mkdir(fileDir, { recursive: true });
  console.log("copying", filePath);
  await Deno.copyFile(asset.source, filePath);
}

async function generatePage(page: Page) {
  const html = await renderPage(page);
  const filePath = path.join(BUILD_DIR, page.path);
  const fileDir = path.dirname(filePath);
  await Deno.mkdir(fileDir, { recursive: true });
  console.log("writing", filePath);
  await Deno.writeTextFile(filePath, html);
}

/**
 * Generates the static site from Markdown based on the `PAGES` and `ASSETS`
 * into the `_build` directory.
 */
async function generateSite() {
  await Deno.mkdir(BUILD_DIR, { recursive: true });
  console.debug("generating pages...");
  await Promise.all(PAGES.map(generatePage));
  await Promise.all(ASSETS.map(copyAsset));
}

function handler(req: Request): Response | Promise<Response> {
  const url = new URL(req.url);
  console.log(`[${req.method}]`, url.pathname);
  const cleanPath = url.pathname.replace(/\/$/, "");
  try {
    const redirect = REDIRECTS[cleanPath];
    if (redirect) {
      let destinationUrl = redirect.dest;

      // paths get the request URL prepended, otherwise assume the
      // dest is a complete URL
      if (redirect.dest.startsWith("/")) {
        destinationUrl = new URL(redirect.dest, url).toString();
      }
      console.debug("redirecting to ", destinationUrl, redirect.status);
      return Response.redirect(destinationUrl, redirect.status);
    }

    return serveDir(req, { fsRoot: "./_build" });
  } catch (err) {
    console.error(err);
    return new Response((err as Error).message, { status: 500 });
  }
}
const port = Deno.env.get("PORT") || "8008";
const args = Deno.args;
const command = args.at(-1);

const errorAndExit = (message: string) => {
  console.error(message);
  Deno.exit(1);
};

if (!command) {
  errorAndExit("Must provide command: serve or build");
}

if (command == "serve") {
  await generateSite();
  Deno.serve({ port: parseInt(port) }, handler);
} else if (command == "build") {
  console.debug("building site...");
  await generateSite();
} else {
  errorAndExit(`Unknown command: ${command}`);
}
