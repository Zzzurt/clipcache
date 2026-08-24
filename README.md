# ClipCache

一个运行在 Windows 上的简洁优雅的剪贴板管理器。自动记录复制的文字、图片与富文本，本地保存，可设定保留时间、到期自动删除。

## 功能

- 📝 记录纯文本（`CF_UNICODETEXT` / `CF_TEXT`）
- 🎨 记录富文本（`CF_HTML` 优先、`CF_RTF` 兜底，支持 HTML 内嵌图片）
- 🖼 记录图片（`PNG` → `CF_DIBV5` → `CF_DIB` → 资源管理器复制文件，统一转存为 PNG）
- 🖼 列表显示图片缩略图
- 📌 置顶记录（永不过期、始终排在最前）
- 💾 本地保存：`clips.json` + `images/`（纯 Rust JSON 存储，无 C 依赖）
- ⏱ 可设定保留时间（24h / 3d / 7d / 30d / 永久），到期自动清理
- 🔎 搜索、按类型过滤、双击写回剪贴板、浅色/深色主题

## 技术栈

- Tauri 2（Rust 后端 + 原生 Win32 FFI）
- 原生 HTML/CSS/JS 前端（无框架、无构建步骤）

## 目录结构

```
src/                前端（index.html / styles.css / main.js）
src-tauri/          Rust 后端
  src/lib.rs        App 构建、状态注入、监听/清理线程
  src/clipboard.rs  剪贴板捕获与回写（原生 FFI）
  src/dib.rs        DIB <-> RGBA 转换
  src/storage.rs    JSON 存储与 CRUD
  src/settings.rs   设置
  src/commands.rs   Tauri 命令
DESIGN.md           设计文档
```

## 开发环境要求

- Node.js ≥ 18
- Rust（stable，`x86_64-pc-windows-msvc`）
- WebView2（Windows 10/11 一般已内置）

## 运行

```bash
npm install
npm run tauri dev
```

## 构建发布版

```bash
npm run tauri build
```

产物位于 `src-tauri/target/release/`，安装包位于 `src-tauri/target/release/bundle/`。

## 数据位置

数据默认保存在应用数据目录（`%APPDATA%\com.clipcache.app\`）：

- `clips.json` — 记录元数据与文本/富文本内容
- `settings.json` — 设置
- `images/` — 图片 PNG 文件
