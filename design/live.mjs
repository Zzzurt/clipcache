// Drives the running ClipCache window over the DevTools protocol and grabs a
// window PNG. Requires the app to be launched with
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
// Driving the page from inside the webview avoids focus/z-order problems
// entirely: clicks are synthesized in JS and CDP captures the surface.
//
// Usage: node live.mjs <out.png> [--row N] [--modal] [--theme dark] [--css path]
import { spawnSync } from "node:child_process";
import { writeFileSync, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const PORT = 9222;

const argv = process.argv.slice(2);
const out = argv[0] || "shots/real-app.png";
const flag = (name, def = null) => {
  const i = argv.indexOf("--" + name);
  return i === -1 ? def : argv[i + 1] ?? true;
};
const row = Number(flag("row", 0));
const modal = argv.includes("--modal");
const theme = flag("theme", null);

const targets = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
const page = targets.find((t) => t.type === "page");
if (!page) throw new Error("no page target; is the app running with CDP enabled?");

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, err) => {
  ws.addEventListener("open", ok, { once: true });
  ws.addEventListener("error", err, { once: true });
});

let seq = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { ok, fail } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? fail(new Error(JSON.stringify(msg.error))) : ok(msg.result);
  }
});
const send = (method, params = {}) =>
  new Promise((ok, fail) => {
    const id = ++seq;
    pending.set(id, { ok, fail });
    ws.send(JSON.stringify({ id, method, params }));
  });

const evaluate = async (expr) => {
  const r = await send("Runtime.evaluate", {
    expression: expr,
    awaitPromise: true,
    returnByValue: true,
  });
  if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
  return r.result?.value;
};

await send("Page.enable");
await send("Runtime.enable");

// The window is already the default 1000x700; resizing it is deliberately not
// attempted because set_size is outside the app's capability ACL.
if (theme) {
  await evaluate(`(() => {
    document.documentElement.dataset.theme = ${JSON.stringify(theme)};
    const sel = document.getElementById("set-theme");
    if (sel) sel.value = ${JSON.stringify(theme)};
    return true;
  })()`);
}

// Wait until the list has rendered (2s poll + async invoke).
await evaluate(`new Promise((done) => {
  const t0 = Date.now();
  (function tick() {
    const rows = document.querySelectorAll(".clip-item");
    if (rows.length > 0 || Date.now() - t0 > 6000) return done(rows.length);
    setTimeout(tick, 100);
  })();
})`);

if (row > 0) {
  // Real DOM click, so main.js handlers run exactly as for a user.
  await evaluate(`(() => {
    const el = document.querySelectorAll(".clip-item")[${row - 1}];
    if (!el) return false;
    el.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    el.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    el.click();
    return true;
  })()`);
  await new Promise((r) => setTimeout(r, 700));
}

if (modal) {
  await evaluate(`document.getElementById("settings-btn").click()`);
  await new Promise((r) => setTimeout(r, 700));
}

await evaluate(`document.querySelectorAll(".clip-item").length`);

// Recompute the frame so the shot matches the window's live size.
const frame = await evaluate(`(async () => {
  const w = window.__TAURI__?.window?.getCurrentWindow?.();
  if (!w) return { w: innerWidth, h: innerHeight };
  const [size, scale] = await Promise.all([w.outerSize(), w.scaleFactor()]);
  return { w: size.width / scale, h: size.height / scale };
})()`);

const shot = await send("Page.captureScreenshot", {
  format: "png",
  captureBeyondViewport: false,
  fromSurface: true,
  clip: { x: 0, y: 0, width: frame.w, height: frame.h, scale: 2 },
});
const buf = Buffer.from(shot.data, "base64");
const dest = resolve(HERE, out);
writeFileSync(dest, buf);
console.log(`saved ${dest} (${frame.w}x${frame.h} @ ${buf.length} B)`);

ws.close();
