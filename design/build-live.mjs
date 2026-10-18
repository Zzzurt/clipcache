// Builds design/mock/live.html: the real src/index.html + real src/main.js,
// with window.__TAURI__ stubbed so the UI renders headless for screenshots.
//
// Set CC_HTML_CLIP=<id> to use a real record from the app's own clips.json as
// the rich-text entry. That is how pasted-page colours get exercised without
// touching the user's data: the fixture is read, never written.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("../src/", import.meta.url);
const html = readFileSync(new URL("index.html", root), "utf8");

const realId = process.env.CC_HTML_CLIP;
let realHtml = "";
let realPreview = "";
let realApp = "";
if (realId) {
  const file = join(process.env.APPDATA, "com.clipcache.app", "clips.json");
  const clip = JSON.parse(readFileSync(file, "utf8")).find(
    (c) => String(c.id) === String(realId)
  );
  if (!clip || !clip.html) throw new Error(`no html clip ${realId}`);
  realHtml = clip.html;
  realPreview = clip.preview || "";
  realApp = clip.source_app || "";
}

const stub = `
<script>
(() => {
  const now = Date.now();
  const CLIPS = [
    { id: 1, kind: "image", preview: "图片 640×400", created_at: now - 3 * 60000, source_app: "Photoshop", pinned: true, byte_size: 1228800, has_thumb: true },
    { id: 2, kind: "image", preview: "图片 520×360", created_at: now - 10 * 60000, source_app: "Figma", pinned: false, byte_size: 640000, has_thumb: true },
    { id: 3, kind: "html", preview: "部署清单 执行数据库迁移 更新环境变量 重启服务并…", created_at: now - 26 * 60000, source_app: "Notion", pinned: false, byte_size: 214, has_thumb: false },
    { id: 4, kind: "text", preview: "SELECT id, title, created_at FROM articles WHERE …", created_at: now - 49 * 60000, source_app: "VS Code", pinned: false, byte_size: 132, has_thumb: false },
    { id: 5, kind: "text", preview: "https://github.com/Zzzurt/clipcache", created_at: now - 3600000, source_app: "Chrome", pinned: false, byte_size: 41, has_thumb: false },
    { id: 6, kind: "text", preview: "✓ 构建成功，耗时 1m 42s", created_at: now - 7200000, source_app: "Terminal", pinned: false, byte_size: 24, has_thumb: false },
  ];
  const thumb = (a, b, c) => {
    const cv = document.createElement("canvas");
    cv.width = 160; cv.height = 160;
    const g = cv.getContext("2d");
    const lg = g.createLinearGradient(0, 0, 160, 160);
    lg.addColorStop(0, a); lg.addColorStop(0.55, b); lg.addColorStop(1, c);
    g.fillStyle = lg; g.fillRect(0, 0, 160, 160);
    g.fillStyle = "rgba(0,0,0,.12)";
    g.beginPath(); g.arc(118, 118, 46, 0, 7); g.fill();
    g.strokeStyle = "rgba(255,255,255,.55)"; g.lineWidth = 2;
    g.beginPath(); g.moveTo(18, 118);
    g.bezierCurveTo(40, 88, 56, 112, 72, 92);
    g.bezierCurveTo(88, 72, 106, 58, 130, 76); g.stroke();
    return cv.toDataURL("image/png").split(",")[1];
  };
  const DETAIL = {
    1: { kind: "image", created_at: now - 3 * 60000, byte_size: 1228800, source_app: "Photoshop", pinned: true,
         image_base64: thumb("#8d7b63", "#c2a07a", "#3f4a52") },
    2: { kind: "image", created_at: now - 10 * 60000, byte_size: 640000, source_app: "Figma", pinned: false,
         image_base64: thumb("#4c5b6b", "#7d8f9c", "#2f3944") },
    3: { kind: "html", created_at: now - 26 * 60000, byte_size: 214, source_app: "Notion", pinned: false,
         html: "<h1>部署清单</h1><ul><li>执行数据库迁移</li><li>更新环境变量</li><li>重启服务并验证健康检查</li></ul>",
         text: "部署清单\\n执行数据库迁移\\n更新环境变量\\n重启服务并验证健康检查" },
    4: { kind: "text", created_at: now - 49 * 60000, byte_size: 132, source_app: "VS Code", pinned: false,
         text: "SELECT id, title, created_at FROM articles WHERE status = 'draft' ORDER BY created_at DESC LIMIT 20;" },
    5: { kind: "text", created_at: now - 3600000, byte_size: 41, source_app: "Chrome", pinned: false,
         text: "https://github.com/Zzzurt/clipcache" },
    6: { kind: "text", created_at: now - 7200000, byte_size: 24, source_app: "Terminal", pinned: false,
         text: "✓ 构建成功，耗时 1m 42s" },
  };
  const handlers = {
    get_settings: () => ({ retention_hours: 168, max_clips: 500, theme: new URLSearchParams(location.search).get("theme") || "light" }),
    list_clips: () => (window.__CC_EXTRA ? CLIPS.concat([window.__CC_EXTRA.clip]) : CLIPS),
    get_stats: () => ({ count: 6, total_bytes: 58160, image_count: 2 }),
    get_clip: ({ id }) => (window.__CC_OVERRIDE && window.__CC_OVERRIDE.id === id) ? window.__CC_OVERRIDE : DETAIL[id],
    get_thumb: ({ id }) => (id === 1 ? thumb("#7b8a6a", "#b98a5e", "#5c4a3d") : thumb("#4c5b6b", "#7d8f9c", "#2f3944")),
    get_data_dir: () => "%APPDATA%\\\\com.clipcache.app",
    get_clipboard_formats: () => { throw new Error("debug only"); },
    open_data_dir: () => {},
    copy_to_clipboard: () => {},
    delete_clip: () => {},
    pin_clip: () => {},
    clear_all: () => 0,
    update_settings: () => {},
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args) => handlers[cmd] ? handlers[cmd](args || {}) : null },
    window: {
      getCurrentWindow: () => ({
        minimize() {}, toggleMaximize() {}, close() {},
        isMaximized: async () => false,
        onResized: async () => {},
      }),
    },
  };
})();
</script>
`;

// A real pasted page, when requested, is exposed to the stub through globals
// rather than inlined into the handler, so the JSON stays out of the code path.
const dataScript = realHtml
  ? `<script>
  (() => {
    const id = 999;
    window.__CC_EXTRA = {
      clip: { id, kind: "html", preview: ${JSON.stringify(realPreview.slice(0, 60))}, created_at: Date.now() - 4 * 60000, source_app: ${JSON.stringify(realApp)}, pinned: true, byte_size: ${Buffer.byteLength(realHtml, "utf8")}, has_thumb: false },
    };
    window.__CC_OVERRIDE = { id, kind: "html", created_at: Date.now() - 4 * 60000, byte_size: ${Buffer.byteLength(realHtml, "utf8")}, source_app: ${JSON.stringify(realApp)}, pinned: true, html: ${JSON.stringify(realHtml)}, text: "" };
  })();
</script>`
  : "";

let out = html
  .replace('<link rel="stylesheet" href="styles.css" />', '<link rel="stylesheet" href="../../src/styles.css" />')
  .replace('<script type="module" src="/main.js" defer></script>',
           stub + dataScript + '\n    <script type="module" src="../../src/main.js"></script>');

// let the harness drive which record is selected
out = out.replace("</body>", `<script>
  const q = new URLSearchParams(location.search);
  const want = q.get("select");
  if (want) {
    const t = setInterval(() => {
      const row = document.querySelector('.clip-item[data-id="' + want + '"]');
      if (row) { clearInterval(t); row.click(); }
    }, 60);
  }
  if (q.get("modal")) {
    const t = setInterval(() => {
      const btn = document.getElementById("settings-btn");
      if (btn) { clearInterval(t); btn.click(); }
    }, 60);
  }
  if (q.get("toast")) {
    const paint = () => {
      const t = document.getElementById("toast");
      if (!t) return;
      t.innerHTML = '<span>已删除</span><button class="toast-undo">撤销</button>';
      t.classList.remove("hidden");
    };
    setInterval(paint, 40);
  }
</script>
</body>`);

writeFileSync(new URL("mock/live.html", import.meta.url), out);
console.log("wrote design/mock/live.html");
