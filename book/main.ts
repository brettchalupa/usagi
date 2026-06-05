import { serveDir, serveFile } from "@std/http/file-server";

// Static file server for the built mdbook output. `mdbook build` writes to
// ./book; that directory is served as-is. Used both locally (`deno task dev`)
// and on Deno Deploy, where the built output is uploaded alongside this file.
const FS_ROOT = "book";

async function handler(req: Request): Promise<Response> {
  const res = await serveDir(req, { fsRoot: FS_ROOT, quiet: true });
  if (res.status === 404) {
    try {
      const notFound = await serveFile(req, `${FS_ROOT}/404.html`);
      return new Response(notFound.body, {
        status: 404,
        headers: { "content-type": "text/html;charset=utf-8" },
      });
    } catch {
      return res;
    }
  }
  return res;
}

const port = parseInt(Deno.env.get("PORT") ?? "8009");
Deno.serve({ port }, handler);
