# ClipCache

一个运行在 Windows 上的简洁优雅的剪贴板管理器。自动记录复制的文字、图片与富文本，本地保存，可设定保留时间、到期自动删除。

## 功能

- 📝 记录纯文本（`CF_UNICODETEXT` / `CF_TEXT`）
- 🎨 记录富文本（`CF_HTML` 优先、`CF_RTF` 兜底，支持 HTML 内嵌图片）
- 🖼 记录图片（`PNG` → `CF_DIBV5` → `CF_DIB` → 资源管理器复制文件，统一转存为 PNG）
- 🖼 列表显示图片缩略图（缩略图独立落盘，不进入列表数据）
- 📌 置顶记录（永不过期、始终排在最前）
- 💾 本地保存：`clips.json` + `images/`（纯 Rust JSON 存储，无 C 依赖）
- ⏱ 可设定保留时间（24h / 3d / 7d / 30d / 永久），到期自动清理
- 🔎 搜索、按类型过滤、双击写回剪贴板、浅色/深色主题
- 🔒 尊重剪贴板隐私标记：标记为机密的复制内容不会被记录

## 隐私

应用会识别 Windows 的剪贴板隐私标记，命中下列任一情况时**静默跳过，不记录任何内容**：

- `Clipboard Viewer Ignore`
- `ExcludeClipboardContentFromMonitorProcessing`
- `CanIncludeInClipboardHistory = 0`
- `CanUploadToCloudClipboard = 0`

密码管理器、浏览器的无痕窗口和 Windows 凭据管理器都会设置这些标记，因此从它们复制的密码不会进入历史记录。

除此之外，记录以**明文**保存在数据目录（见下文）。没有内置加密，请自行确保该目录的访问权限，或缩短保留时间。

### 预览会加载远程图片

富文本预览在 `sandbox` iframe 中渲染，**脚本被禁止**，但 https 图片会被加载——这样从网页复制的表情和配图才能正常显示。代价是：打开一条含远程图片的记录时，应用会向图片所在站点发起请求，对方可能由此得知你的 IP 以及「这条内容被查看过」。

如果你不希望产生这类外部请求，可以断开网络，或只处理不依赖远程图片的内容。

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
- `images/` — 图片 PNG 文件，以及 `thumb_<id>.png` 缩略图

可用 `CLIPCACHE_DATA_DIR` 环境变量覆盖该位置。

## 已知限制

- 最多保留 1000 条记录（列表与 `max_clips` 设置的上限一致，超出部分会被裁剪）。
- 数据不加密，见上文「隐私」。
- 无托盘图标、无开机自启。
- 富文本预览在 `sandbox` iframe 中渲染，且禁止加载远程图片，因此预览里的外链图片不会显示。

## 开发

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # 单元测试
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

两个写真实系统剪贴板的测试默认被标记为 `#[ignore]`，需要时用 `cargo test -- --ignored` 手动运行。CI 只跑不触碰剪贴板的测试。

