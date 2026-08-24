const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const ICONS = {
  text: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h6"/></svg>`,
  html: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 7l-5 5 5 5"/><path d="M16 7l5 5-5 5"/><path d="M13 5l-2 14"/></svg>`,
  rtf: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M9 12h6M9 16h6"/></svg>`,
  image: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="1.5"/><path d="M4 19l5-5 4 4 3-3 4 4"/></svg>`,
};

const TYPE_LABEL = {
  text: "文本",
  html: "富文本",
  rtf: "RTF",
  image: "图片",
};

const state = {
  clips: [],
  selectedId: null,
  detail: null,
  kind: "",
  search: "",
  settings: null,
  signature: "",
};

// ids pending soft-deletion (awaiting the undo window)
const pendingDeletion = new Map();

const $ = (id) => document.getElementById(id);

function iconFor(kind) {
  return ICONS[kind] || ICONS.text;
}

function relativeTime(ms) {
  const diff = Date.now() - ms;
  const sec = Math.floor(diff / 1000);
  if (sec < 10) return "刚刚";
  if (sec < 60) return `${sec} 秒前`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} 小时前`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} 天前`;
  return new Date(ms).toLocaleDateString("zh-CN");
}

function fmtBytes(b) {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

function showToast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  clearTimeout(t._timer);
  t._timer = setTimeout(() => t.classList.add("hidden"), 1500);
}

function showUndoToast(msg, onUndo) {
  const t = $("toast");
  t.innerHTML = "";
  const span = document.createElement("span");
  span.textContent = msg;
  const btn = document.createElement("button");
  btn.className = "toast-undo";
  btn.textContent = "撤销";
  btn.addEventListener("click", () => {
    clearTimeout(t._timer);
    t.classList.add("hidden");
    onUndo();
  });
  t.appendChild(span);
  t.appendChild(btn);
  t.classList.remove("hidden");
  clearTimeout(t._timer);
  t._timer = setTimeout(() => t.classList.add("hidden"), 3000);
}

function applyTheme(theme) {
  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");
}

// ---------------- data ----------------

async function loadSettings() {
  state.settings = await invoke("get_settings");
  applyTheme(state.settings.theme);
}

async function refreshList() {
  let clips = await invoke("list_clips", {
    limit: 1000,
    offset: 0,
    kind: state.kind,
    search: state.search,
  });
  // hide items pending soft-deletion until the undo window closes
  if (pendingDeletion.size) {
    clips = clips.filter((c) => !pendingDeletion.has(c.id));
  }
  const sig = `${clips.length}:${clips.length ? clips[0].id : "x"}`;
  state.clips = clips;
  if (sig !== state.signature) {
    state.signature = sig;
    renderList();
    // if selected clip vanished, reset preview
    if (
      state.selectedId != null &&
      !clips.some((c) => c.id === state.selectedId)
    ) {
      resetPreview();
    }
  }
  updateStats();
}

async function updateStats() {
  const s = await invoke("get_stats");
  let text = `${s.count} 条记录 · ${fmtBytes(s.total_bytes)}`;
  if (s.image_count) text += ` · ${s.image_count} 张图片`;
  $("stats").textContent = text;
}

// ---------------- render list ----------------

function renderList() {
  const list = $("list");
  list.innerHTML = "";
  const empty = $("empty-state");
  if (state.clips.length === 0) {
    empty.classList.remove("hidden");
    return;
  }
  empty.classList.add("hidden");

  for (const c of state.clips) {
    const item = document.createElement("div");
    item.className = "clip-item" + (c.id === state.selectedId ? " selected" : "");
    item.dataset.id = c.id;

    const icon = document.createElement("div");
    icon.className = "clip-icon";
    if (c.kind === "image" && c.thumb_base64) {
      const img = document.createElement("img");
      img.className = "clip-thumb";
      img.src = `data:image/png;base64,${c.thumb_base64}`;
      icon.appendChild(img);
    } else {
      icon.innerHTML = iconFor(c.kind);
    }

    const body = document.createElement("div");
    body.className = "clip-body";

    const preview = document.createElement("div");
    preview.className = "clip-preview" + (c.kind === "image" ? " is-image" : "");
    preview.textContent = c.preview || "(空)";

    const meta = document.createElement("div");
    meta.className = "clip-meta";
    if (c.pinned) {
      const pin = document.createElement("span");
      pin.className = "clip-pin-badge";
      pin.textContent = "置顶";
      meta.appendChild(pin);
    }
    const time = document.createElement("span");
    time.textContent = relativeTime(c.created_at);
    meta.appendChild(time);
    if (c.source_app) {
      const app = document.createElement("span");
      app.className = "clip-app";
      app.textContent = c.source_app;
      meta.appendChild(app);
    }

    body.appendChild(preview);
    body.appendChild(meta);

    const del = document.createElement("button");
    del.className = "clip-delete";
    del.title = "删除";
    del.textContent = "×";

    item.appendChild(icon);
    item.appendChild(body);
    item.appendChild(del);
    list.appendChild(item);
  }
}

// ---------------- preview ----------------

function resetPreview() {
  state.selectedId = null;
  $("preview-placeholder").classList.remove("hidden");
  $("preview-content").classList.add("hidden");
}

async function selectClip(id) {
  state.selectedId = id;
  // update highlight immediately
  document.querySelectorAll(".clip-item").forEach((el) => {
    el.classList.toggle("selected", Number(el.dataset.id) === id);
  });
  const d = await invoke("get_clip", { id });
  if (!d) return;
  state.detail = d;
  renderDetail(d);
}

function renderDetail(d) {
  $("preview-placeholder").classList.add("hidden");
  $("preview-content").classList.remove("hidden");
  $("pv-type").textContent = TYPE_LABEL[d.kind] || d.kind;
  $("pv-meta").textContent = `${relativeTime(d.created_at)} · ${fmtBytes(
    d.byte_size
  )}${d.source_app ? " · 来自 " + d.source_app : ""}`;
  updatePinButton(d.pinned);

  const body = $("pv-body");
  body.innerHTML = "";

  // Image first (a combined text+image clip shows both)
  if (d.image_base64) {
    const wrap = document.createElement("div");
    wrap.className = "image-wrap";
    const img = document.createElement("img");
    img.className = "preview-image";
    img.src = `data:image/png;base64,${d.image_base64}`;
    wrap.appendChild(img);
    body.appendChild(wrap);
  }

  if (d.kind === "html" && d.html) {
    const iframe = document.createElement("iframe");
    iframe.className = "preview-frame";
    iframe.setAttribute("sandbox", "");
    const doc = `<!doctype html><html><head><meta charset="utf-8"><style>body{font-family:-apple-system,'Segoe UI',system-ui,sans-serif;font-size:14px;line-height:1.6;margin:0;padding:16px;color:#1c1f26;word-break:break-word;}</style></head><body>${d.html}</body></html>`;
    iframe.srcdoc = doc;
    body.appendChild(iframe);
  } else if (d.text) {
    const pre = document.createElement("pre");
    pre.className = "preview-text";
    pre.textContent = d.text;
    body.appendChild(pre);
  }

  if (body.children.length === 0) {
    const pre = document.createElement("pre");
    pre.textContent = d.preview || "(无内容)";
    body.appendChild(pre);
  }
}

function updatePinButton(pinned) {
  const btn = $("pv-pin");
  btn.textContent = pinned ? "取消置顶" : "置顶";
  btn.classList.toggle("active", !!pinned);
}

// ---------------- actions ----------------

async function copyClip(id) {
  try {
    await invoke("copy_to_clipboard", { id });
    showToast("已复制到剪贴板");
    refreshList();
  } catch (e) {
    showToast("复制失败：" + e);
  }
}

function deleteClip(id) {
  if (state.selectedId === id) resetPreview();
  // soft delete: hide immediately, real delete after the undo window
  state.clips = state.clips.filter((c) => c.id !== id);
  renderList();
  updateStats();
  pendingDeletion.set(
    id,
    setTimeout(async () => {
      pendingDeletion.delete(id);
      await invoke("delete_clip", { id });
      refreshList();
    }, 3000)
  );
  showUndoToast("已删除", () => {
    const t = pendingDeletion.get(id);
    if (t) {
      clearTimeout(t);
      pendingDeletion.delete(id);
    }
    refreshList();
  });
}

async function togglePin() {
  if (state.selectedId == null || !state.detail) return;
  const id = state.selectedId;
  const pinned = !state.detail.pinned;
  await invoke("pin_clip", { id, pinned });
  showToast(pinned ? "已置顶（永不过期）" : "已取消置顶");
  state.signature = "";
  await refreshList();
  const d = await invoke("get_clip", { id });
  if (d) {
    state.detail = d;
    renderDetail(d);
  }
}

async function clearAll() {
  if (!confirm("确定要清空所有记录吗？此操作不可撤销。")) return;
  const n = await invoke("clear_all");
  resetPreview();
  refreshList();
  showToast(`已清空 ${n} 条记录`);
}

// ---------------- settings ----------------

async function openSettings() {
  const s = state.settings;
  $("set-retention").value = s.retention_hours == null ? "forever" : String(s.retention_hours);
  $("set-max").value = s.max_clips;
  $("set-theme").value = s.theme;
  $("set-apply").checked = false;
  $("data-dir-path").textContent = await invoke("get_data_dir");
  $("settings-modal").classList.remove("hidden");
}

function closeSettings() {
  $("settings-modal").classList.add("hidden");
}

async function saveSettings() {
  const rv = $("set-retention").value;
  const retention = rv === "forever" ? null : Number(rv);
  const maxClips = Number($("set-max").value) || 500;
  const theme = $("set-theme").value;
  const applyExisting = $("set-apply").checked;

  await invoke("update_settings", {
    retentionHours: retention,
    maxClips,
    theme,
    applyExisting,
  });
  await loadSettings();
  closeSettings();
  refreshList();
  showToast("设置已保存");
}

// ---------------- keyboard ----------------

function currentIndex() {
  if (state.selectedId == null) return -1;
  return state.clips.findIndex((c) => c.id === state.selectedId);
}

function moveSelection(delta) {
  if (state.clips.length === 0) return;
  const idx = currentIndex();
  let next;
  if (idx === -1) {
    next = delta > 0 ? 0 : state.clips.length - 1;
  } else {
    next = idx + delta;
    if (next < 0) next = state.clips.length - 1;
    if (next >= state.clips.length) next = 0;
  }
  const clip = state.clips[next];
  selectClip(clip.id);
  const el = document.querySelector(`.clip-item[data-id="${clip.id}"]`);
  if (el) el.scrollIntoView({ block: "nearest" });
}

function handleKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && (e.key === "f" || e.key === "k")) {
    e.preventDefault();
    const s = $("search");
    s.focus();
    s.select();
    return;
  }

  if (e.key === "Escape") {
    if (!$("settings-modal").classList.contains("hidden")) {
      closeSettings();
    } else if (document.activeElement === $("search")) {
      $("search").blur();
    }
    return;
  }

  const tag = document.activeElement ? document.activeElement.tagName : "";
  if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;

  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveSelection(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveSelection(-1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (state.selectedId != null) copyClip(state.selectedId);
  } else if (e.key === "Delete") {
    e.preventDefault();
    if (state.selectedId != null) deleteClip(state.selectedId);
  }
}

// ---------------- wiring ----------------

function bindEvents() {
  $("search").addEventListener("input", () => {
    clearTimeout(state._debounce);
    state._debounce = setTimeout(() => {
      state.search = $("search").value.trim();
      state.signature = "";
      refreshList();
    }, 200);
  });

  $("filters").addEventListener("click", (e) => {
    const btn = e.target.closest(".filter");
    if (!btn) return;
    document.querySelectorAll(".filter").forEach((f) => f.classList.remove("active"));
    btn.classList.add("active");
    state.kind = btn.dataset.kind;
    state.signature = "";
    refreshList();
  });

  $("list").addEventListener("click", (e) => {
    const delBtn = e.target.closest(".clip-delete");
    if (delBtn) {
      e.stopPropagation();
      deleteClip(Number(delBtn.closest(".clip-item").dataset.id));
      return;
    }
    const item = e.target.closest(".clip-item");
    if (item) selectClip(Number(item.dataset.id));
  });

  $("list").addEventListener("dblclick", (e) => {
    const item = e.target.closest(".clip-item");
    if (item) copyClip(Number(item.dataset.id));
  });

  $("pv-copy").addEventListener("click", () => {
    if (state.selectedId != null) copyClip(state.selectedId);
  });
  $("pv-delete").addEventListener("click", () => {
    if (state.selectedId != null) deleteClip(state.selectedId);
  });
  $("pv-pin").addEventListener("click", togglePin);

  $("clear-btn").addEventListener("click", clearAll);
  $("settings-btn").addEventListener("click", openSettings);
  $("set-cancel").addEventListener("click", closeSettings);
  $("set-save").addEventListener("click", saveSettings);
  $("settings-modal").addEventListener("click", (e) => {
    if (e.target === $("settings-modal")) closeSettings();
  });
  $("dbg-formats").addEventListener("click", async () => {
    try {
      const fmts = await invoke("get_clipboard_formats");
      $("dbg-output").textContent =
        fmts
          .map((f) => `${String(f.id).padStart(2)} ${f.name} (${fmtBytes(f.size)})`)
          .join("\n") || "(剪贴板为空)";
    } catch (e) {
      $("dbg-output").textContent = "读取失败：" + e;
    }
  });
  $("open-data-dir").addEventListener("click", () => invoke("open_data_dir"));

  // Custom title bar window controls
  const win = getCurrentWindow();
  const MAX_ICON =
    '<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>';
  const RESTORE_ICON =
    '<svg viewBox="0 0 10 10"><path d="M2.5 2.5V0.5h7v7h-2" fill="none" stroke="currentColor" stroke-width="1.2"/><rect x="0.5" y="2.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>';
  $("win-min").addEventListener("click", () => win.minimize());
  $("win-max").addEventListener("click", () => win.toggleMaximize());
  $("win-close").addEventListener("click", () => win.close());
  const updateMaxIcon = async () => {
    try {
      const max = await win.isMaximized();
      $("win-max").innerHTML = max ? RESTORE_ICON : MAX_ICON;
    } catch (e) {
      /* ignore */
    }
  };
  win.onResized(updateMaxIcon).catch(() => {});

  window.addEventListener("keydown", handleKeydown);
}

async function init() {
  bindEvents();
  await loadSettings();
  await refreshList();
  setInterval(refreshList, 2000);
}

window.addEventListener("DOMContentLoaded", init);
