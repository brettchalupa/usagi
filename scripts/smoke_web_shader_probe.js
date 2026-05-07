#!/usr/bin/env node
// CDP probe for the web shader smoke test. It drives the click-to-play
// overlay, validates WebGL/shader logs, cycles from CRT to Game Boy, and
// writes screenshots plus a JSON report under target/.

const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");

function usage() {
  console.error(
    "Usage: node scripts/smoke_web_shader_probe.js --url URL --debug-port PORT --out-dir DIR",
  );
}

function parseArgs(argv) {
  const args = {
    url: "",
    debugPort: 9223,
    outDir: path.join("target", "web-shader-smoke"),
  };
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--url") {
      args.url = argv[++i] || "";
    } else if (arg === "--debug-port") {
      args.debugPort = Number(argv[++i]);
    } else if (arg === "--out-dir") {
      args.outDir = argv[++i] || args.outDir;
    } else if (arg === "-h" || arg === "--help") {
      usage();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!args.url || !Number.isInteger(args.debugPort)) {
    usage();
    process.exit(2);
  }
  return args;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function getPage(debugPort) {
  const pages = await fetch(`http://127.0.0.1:${debugPort}/json/list`).then((r) =>
    r.json()
  );
  const page = pages.find((p) => p.type === "page") || pages[0];
  if (!page) {
    throw new Error("no Chrome page target found");
  }
  return page;
}

async function connect(page) {
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve, { once: true });
    ws.addEventListener("error", reject, { once: true });
  });
  return ws;
}

function createCdpClient(ws, events) {
  let nextId = 1;
  const pending = new Map();

  ws.addEventListener("message", (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) {
        reject(new Error(`${msg.error.code}: ${msg.error.message}`));
      } else {
        resolve(msg.result || {});
      }
      return;
    }

    if (msg.method === "Runtime.consoleAPICalled") {
      events.consoleMessages.push({
        type: msg.params.type,
        text: msg.params.args
          .map((arg) => arg.value ?? arg.description ?? "")
          .join(" "),
      });
    } else if (msg.method === "Runtime.exceptionThrown") {
      events.exceptions.push(
        msg.params.exceptionDetails?.text || JSON.stringify(msg.params),
      );
    } else if (msg.method === "Log.entryAdded") {
      const entry = msg.params.entry;
      events.logEntries.push({
        level: entry.level,
        source: entry.source,
        text: entry.text,
        url: entry.url || "",
      });
    } else if (msg.method === "Network.loadingFailed") {
      events.failedRequests.push({
        requestId: msg.params.requestId,
        type: msg.params.type,
        errorText: msg.params.errorText,
        canceled: msg.params.canceled,
      });
    } else if (msg.method === "Page.loadEventFired") {
      events.loadFired = true;
    }
  });

  return {
    send(method, params = {}) {
      const id = nextId++;
      ws.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
      });
    },
  };
}

async function waitFor(client, expression, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;
  while (Date.now() < deadline) {
    const result = await client.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
    });
    lastValue = result.result?.value;
    if (lastValue) {
      return lastValue;
    }
    await sleep(100);
  }
  throw new Error(`${label} timed out; last value: ${JSON.stringify(lastValue)}`);
}

async function capture(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png" });
  fs.writeFileSync(filePath, Buffer.from(shot.data, "base64"));
}

function countConsole(events, needle) {
  return events.consoleMessages.filter((msg) => msg.text.includes(needle)).length;
}

function paethPredictor(left, up, upLeft) {
  const p = left + up - upLeft;
  const pa = Math.abs(p - left);
  const pb = Math.abs(p - up);
  const pc = Math.abs(p - upLeft);
  if (pa <= pb && pa <= pc) return left;
  if (pb <= pc) return up;
  return upLeft;
}

function decodePng(filePath) {
  const png = fs.readFileSync(filePath);
  if (png.toString("ascii", 1, 4) !== "PNG") {
    throw new Error(`${filePath}: not a PNG file`);
  }

  let offset = 8;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  const idat = [];

  while (offset < png.length) {
    const len = png.readUInt32BE(offset);
    const type = png.toString("ascii", offset + 4, offset + 8);
    const data = png.subarray(offset + 8, offset + 8 + len);
    offset += 12 + len;

    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
    } else if (type === "IDAT") {
      idat.push(data);
    } else if (type === "IEND") {
      break;
    }
  }

  if (bitDepth !== 8 || ![2, 6].includes(colorType)) {
    throw new Error(`${filePath}: unsupported PNG format bitDepth=${bitDepth} colorType=${colorType}`);
  }

  const channels = colorType === 6 ? 4 : 3;
  const stride = width * channels;
  const inflated = zlib.inflateSync(Buffer.concat(idat));
  const pixels = Buffer.alloc(width * height * channels);
  let src = 0;
  let dst = 0;
  let prev = Buffer.alloc(stride);

  for (let y = 0; y < height; y += 1) {
    const filter = inflated[src++];
    const row = Buffer.from(inflated.subarray(src, src + stride));
    src += stride;

    for (let x = 0; x < stride; x += 1) {
      const left = x >= channels ? row[x - channels] : 0;
      const up = prev[x] || 0;
      const upLeft = x >= channels ? prev[x - channels] || 0 : 0;
      let add = 0;
      if (filter === 1) add = left;
      else if (filter === 2) add = up;
      else if (filter === 3) add = Math.floor((left + up) / 2);
      else if (filter === 4) add = paethPredictor(left, up, upLeft);
      else if (filter !== 0) throw new Error(`${filePath}: unsupported PNG filter ${filter}`);
      row[x] = (row[x] + add) & 0xff;
    }

    row.copy(pixels, dst);
    dst += stride;
    prev = row;
  }

  return { width, height, channels, pixels };
}

function readPngStats(filePath) {
  const image = decodePng(filePath);
  let r = 0;
  let g = 0;
  let b = 0;
  let nonBlack = 0;
  let samples = 0;
  const stride = 8;
  for (let y = 0; y < image.height; y += stride) {
    for (let x = 0; x < image.width; x += stride) {
      const off = (y * image.width + x) * image.channels;
      const pr = image.pixels[off];
      const pg = image.pixels[off + 1];
      const pb = image.pixels[off + 2];
      r += pr;
      g += pg;
      b += pb;
      if (pr > 8 || pg > 8 || pb > 8) nonBlack += 1;
      samples += 1;
    }
  }
  return {
    width: image.width,
    height: image.height,
    avgR: r / samples,
    avgG: g / samples,
    avgB: b / samples,
    nonBlackRatio: nonBlack / samples,
    samples,
  };
}

function ignoredLogError(entry) {
  return entry.url.endsWith("/favicon.ico") && entry.text.includes("404");
}

function assertSmoke(report) {
  const failures = [];
  const page = report.page;
  if (page.title !== "Shader demo") {
    failures.push(`expected title "Shader demo", got "${page.title}"`);
  }
  if (page.overlayDisplay !== "none") {
    failures.push(`expected overlay hidden, got display=${page.overlayDisplay}`);
  }
  if (!page.webgl) {
    failures.push("WebGL context was not available");
  }
  if (page.glError !== 0) {
    failures.push(`WebGL error after shader run: ${page.glError}`);
  }
  if (!report.sawGlslEs100) {
    failures.push("did not see GLSL ES 1.00 in browser GL log");
  }
  if (report.fragmentCompileCount < 3) {
    failures.push(
      `expected at least 3 fragment shader compiles, saw ${report.fragmentCompileCount}`,
    );
  }
  if (report.programLoadCount < 3) {
    failures.push(
      `expected at least 3 shader program loads, saw ${report.programLoadCount}`,
    );
  }
  if (!report.canvasStats.crt || !report.canvasStats.gameboy) {
    failures.push("could not sample rendered canvas pixels");
  } else {
    const crt = report.canvasStats.crt;
    const gameboy = report.canvasStats.gameboy;
    const delta =
      Math.abs(crt.avgR - gameboy.avgR) +
      Math.abs(crt.avgG - gameboy.avgG) +
      Math.abs(crt.avgB - gameboy.avgB);
    if (crt.nonBlackRatio < 0.2 || gameboy.nonBlackRatio < 0.2) {
      failures.push(
        `rendered frame looked mostly blank: crt=${crt.nonBlackRatio}, gameboy=${gameboy.nonBlackRatio}`,
      );
    }
    if (delta < 20) {
      failures.push(
        `CRT and Game Boy frames were too similar: color delta ${delta.toFixed(2)}`,
      );
    }
    if (!(gameboy.avgG > gameboy.avgR * 1.15 && gameboy.avgG > gameboy.avgB * 1.15)) {
      failures.push(
        `Game Boy frame was not green-dominant: avg rgb ${gameboy.avgR.toFixed(1)}, ${gameboy.avgG.toFixed(1)}, ${gameboy.avgB.toFixed(1)}`,
      );
    }
  }
  if (report.exceptions.length > 0) {
    failures.push(`runtime exceptions: ${report.exceptions.join(" | ")}`);
  }
  if (report.failedRequests.length > 0) {
    failures.push(`failed network requests: ${JSON.stringify(report.failedRequests)}`);
  }
  const logErrors = report.logEntries.filter(
    (entry) => entry.level === "error" && !ignoredLogError(entry),
  );
  if (logErrors.length > 0) {
    failures.push(`browser log errors: ${JSON.stringify(logErrors)}`);
  }
  if (failures.length > 0) {
    throw new Error(failures.join("\n"));
  }
}

async function run() {
  if (typeof WebSocket !== "function") {
    throw new Error("Node.js with global WebSocket support is required (Node 22+).");
  }

  const args = parseArgs(process.argv);
  fs.mkdirSync(args.outDir, { recursive: true });

  const events = {
    consoleMessages: [],
    logEntries: [],
    exceptions: [],
    failedRequests: [],
    loadFired: false,
  };

  const page = await getPage(args.debugPort);
  const ws = await connect(page);
  const client = createCdpClient(ws, events);

  try {
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await client.send("Emulation.setDeviceMetricsOverride", {
      width: 960,
      height: 540,
      deviceScaleFactor: 1,
      mobile: false,
    });

    await client.send("Page.navigate", { url: args.url });
    await waitFor(
      client,
      "document.readyState === 'complete' && document.getElementById('start') && !document.getElementById('start').disabled",
      20000,
      "runtime ready",
    );

    await client.send("Input.dispatchMouseEvent", {
      type: "mousePressed",
      x: 480,
      y: 270,
      button: "left",
      clickCount: 1,
    });
    await client.send("Input.dispatchMouseEvent", {
      type: "mouseReleased",
      x: 480,
      y: 270,
      button: "left",
      clickCount: 1,
    });

    await waitFor(
      client,
      "document.title === 'Shader demo' && getComputedStyle(document.getElementById('overlay')).display === 'none'",
      20000,
      "shader demo startup",
    );
    await waitFor(
      client,
      "(() => { const canvas = document.querySelector('canvas'); return !!(canvas && (canvas.getContext('webgl') || canvas.getContext('webgl2'))); })()",
      5000,
      "WebGL context",
    );

    await sleep(1000);
    const crtScreenshot = path.join(args.outDir, "crt.png");
    await capture(client, crtScreenshot);
    const crtStats = readPngStats(crtScreenshot);

    const beforeGameboyPrograms = countConsole(events, "Program shader loaded successfully");
    await client.send("Runtime.evaluate", {
      expression:
        "(() => { const c = document.querySelector('canvas'); c && c.focus(); const down = new KeyboardEvent('keydown', { key: 'z', code: 'KeyZ', keyCode: 90, which: 90, bubbles: true, cancelable: true }); const up = new KeyboardEvent('keyup', { key: 'z', code: 'KeyZ', keyCode: 90, which: 90, bubbles: true, cancelable: true }); window.dispatchEvent(down); document.dispatchEvent(down); setTimeout(() => { window.dispatchEvent(up); document.dispatchEvent(up); }, 250); return true; })()",
      returnByValue: true,
    });

    const programDeadline = Date.now() + 8000;
    while (
      countConsole(events, "Program shader loaded successfully") <=
        beforeGameboyPrograms &&
      Date.now() < programDeadline
    ) {
      await sleep(100);
    }

    await sleep(1000);
    const gameboyScreenshot = path.join(args.outDir, "gameboy.png");
    await capture(client, gameboyScreenshot);
    const gameboyStats = readPngStats(gameboyScreenshot);

    const pageState = await client.send("Runtime.evaluate", {
      expression:
        "(() => { const canvas = document.querySelector('canvas'); const overlay = document.getElementById('overlay'); const gl = canvas && (canvas.getContext('webgl') || canvas.getContext('webgl2')); return { title: document.title, overlayDisplay: overlay ? getComputedStyle(overlay).display : null, canvasCount: document.querySelectorAll('canvas').length, canvasWidth: canvas ? canvas.width : null, canvasHeight: canvas ? canvas.height : null, clientWidth: canvas ? canvas.clientWidth : null, clientHeight: canvas ? canvas.clientHeight : null, webgl: !!gl, glError: gl ? gl.getError() : null }; })()",
      returnByValue: true,
    });

    const report = {
      page: pageState.result.value,
      sawGlslEs100: events.consoleMessages.some((msg) =>
        msg.text.includes("GLSL ES 1.00")
      ),
      fragmentCompileCount: countConsole(
        events,
        "Fragment shader compiled successfully",
      ),
      programLoadCount: countConsole(events, "Program shader loaded successfully"),
      canvasStats: {
        crt: crtStats,
        gameboy: gameboyStats,
      },
      consoleMessages: events.consoleMessages,
      logEntries: events.logEntries,
      exceptions: events.exceptions,
      failedRequests: events.failedRequests,
      screenshots: {
        crt: crtScreenshot,
        gameboy: gameboyScreenshot,
      },
    };
    assertSmoke(report);

    const reportPath = path.join(args.outDir, "report.json");
    fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(`[usagi] web shader smoke passed: ${reportPath}`);
    console.log(`[usagi] screenshots: ${crtScreenshot}, ${gameboyScreenshot}`);
  } finally {
    ws.close();
  }
}

run().catch((err) => {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
});
