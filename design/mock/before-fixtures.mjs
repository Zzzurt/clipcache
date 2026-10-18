// Fixtures for mock/before.html: reproduces what src/main.js rendered in the
// pre-redesign UI, without Tauri. Kept as a separate module so build-before.mjs
// can inline it into the generated page.
const T = (min) => new Date(Date.now() - min * 60000);
const rel = (d) => {
  const m = Math.round((Date.now() - d.getTime()) / 60000);
  if (m < 1) return "刚刚";
  if (m < 60) return m + " 分钟前";
  const h = Math.round(m / 60);
  if (h < 24) return h + " 小时前";
  return Math.round(h / 24) + " 天前";
};
const svg = (body) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
const ICON = {
  text: svg('<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h6"/>'),
  html: svg('<path d="M8 7l-5 5 5 5"/><path d="M16 7l5 5-5 5"/><path d="M13 5l-2 14"/>'),
  image: svg('<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="1.5"/><path d="M4 19l5-5 4 4 3-3 4 4"/>'),
};

const grad = (a, b) => {
  const c = document.createElement("canvas");
  c.width = 64;
  c.height = 48;
  const g = c.getContext("2d");
  const lg = g.createLinearGradient(0, 0, 64, 48);
  lg.addColorStop(0, a);
  lg.addColorStop(1, b);
  g.fillStyle = lg;
  g.fillRect(0, 0, 64, 48);
  return c.toDataURL();
};

const CLIPS = [
  {
    id: 1, kind: "image", preview: "图片 640×400", app: "Photoshop",
    at: T(3), ts: grad("#c9a27a", "#7d5a45"), pinned: true,
  },
  {
    id: 2, kind: "image", preview: "图片 520×360", app: "Figma",
    at: T(10), ts: grad("#8f93a8", "#3f4356"), pinned: false,
  },
  {
    id: 3, kind: "html", preview: "部署清单 执行数据库迁移 更新环境变量 重启…", app: "Notion",
    at: T(26), ts: null, pinned: false,
  },
  {
    id: 4, kind: "text", preview: "SELECT id, title, created_at FROM articles WH…", app: "VS Code",
    at: T(49), ts: null, pinned: false,
  },
  {
    id: 5, kind: "text", preview: "https://github.com/Zzzurt/clipcache", app: "Chrome",
    at: T(60), ts: null, pinned: false,
  },
  {
    id: 6, kind: "text", preview: "✓ 构建成功，耗时 1m 42s", app: "Terminal",
    at: T(120), ts: null, pinned: false,
  },
];

const list = document.getElementById("list");
list.innerHTML = CLIPS.map((c) => {
  const sel = c.id === 3 ? " selected" : "";
  const icon = c.ts ? `<img class="clip-thumb" src="${c.ts}" alt="">` : (ICON[c.kind] || ICON.text);
  return `<div class="clip-item${sel}" data-id="${c.id}">
    <div class="clip-icon">${icon}</div>
    <div class="clip-body">
      <div class="clip-preview${c.kind === "image" ? " is-image" : ""}">${c.preview}</div>
      <div class="clip-meta">
        ${c.pinned ? '<span class="clip-pin-badge">置顶</span>' : ""}
        <span>${rel(c.at)}</span>
        <span class="clip-app">${c.app}</span>
      </div>
    </div>
    <button class="clip-delete" title="删除">×</button>
  </div>`;
}).join("");

document.getElementById("stats").textContent = "6 条记录 · 56.8 KB · 2 张图片";
document.getElementById("preview-placeholder").classList.add("hidden");
const pc = document.getElementById("preview-content");
pc.classList.remove("hidden");
document.getElementById("pv-type").textContent = "富文本";
document.getElementById("pv-meta").textContent = `27 分钟前 · 214 B · 来自 Notion`;
document.getElementById("pv-pin").textContent = "置顶";
document.getElementById("pv-body").innerHTML =
  `<h3 style="margin:0 0 14px;font-size:16px">部署清单</h3>
   <ul style="margin:0;padding-left:22px;line-height:2">
     <li>执行数据库迁移</li><li>更新环境变量</li><li>重启服务并验证健康检查</li>
   </ul>`;

const theme = new URLSearchParams(location.search).get("theme");
if (theme) document.documentElement.dataset.theme = theme;
