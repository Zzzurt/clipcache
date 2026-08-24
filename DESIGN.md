# ClipCache — Windows 剪贴板管理器 设计文档

> 记录复制的文字、图片与富文本，本地保存，可设定保留时间、到期自动删除，UI 简洁优雅。

## 1. 目标与范围

| 需求 | 实现方式 |
| --- | --- |
| 记录文字 | 捕获剪贴板 `CF_UNICODETEXT` / `CF_TEXT` |
| 记录富文本 | 捕获 `HTML Format`（CF_HTML）与 `Rich Text Format`（CF_RTF），优先 HTML 展示 |
| 记录图片 | 捕获 `PNG`（注册格式）→ `CF_DIBV5` → `CF_DIB`，统一转存为 PNG 文件 |
| 本地保存 | `clips.json` 存元数据与文本/HTML/RTF，图片以 PNG 文件落盘 |
| 可设定保留时间 | 全局设置（24h / 3d / 7d / 30d / 永久），写入时计算 `expires_at` |
| 到期自动删除 | 启动时 + 每 5 分钟清理过期记录及其图片文件 |
| UI 简洁优雅 | 单窗口：搜索 + 类型过滤 + 时间倒序列表 + 预览区，浅色/深色主题 |

## 2. 技术选型

- **Tauri 2.x**：Rust 后端 + Web 前端，体积小（几 MB）、内存占用低、可直接调用 Win32 API。
- **前端**：原生 HTML/CSS/JS（无框架、无构建步骤），纯 CSS 实现优雅视觉。
- **存储**：纯 Rust 的 JSON 文件存储（`serde_json`），不引入 C 编译依赖；图片以 PNG 文件落盘。
- **Win32 集成**：直接声明原生 FFI（`#[link]`）调用 user32/kernel32 的剪贴板 API，无第三方绑定、签名完全可控。
- **图片编码**：`image` crate（仅启用 `png` 特性，DIB ↔ PNG）。

> 说明：之所以放弃 `rusqlite`（bundled），是因为目标环境缺少 C 编译器，bundled SQLite 需要编译 C 源码；JSON 文件存储对剪贴板管理器规模（数百~数千条）完全够用，且零依赖、崩溃安全。

## 3. 架构总览

```
┌─────────────────────────────────────────────────────────┐
│  Web 前端 (HTML/CSS/JS)                                  │
│  列表 / 搜索 / 过滤 / 预览 / 设置                        │
└──────────────────────────┬──────────────────────────────┘
                           │ Tauri IPC (invoke)
┌──────────────────────────▼──────────────────────────────┐
│  Rust 后端                                              │
│  ┌───────────────┐  ┌────────────┐  ┌────────────────┐  │
│  │ 监听线程(轮询) │─▶│ 数据提取    │─▶│ storage 入库    │  │
│  │ SequenceNumber│  │ text/html/ │  │ clips.json +    │  │
│  └───────────────┘  │ rtf/image  │  │ images/*.png    │  │
│                     └────────────┘  └───────┬────────┘  │
│  ┌───────────────┐                          │            │
│  │ 清理线程       │◀─────────────────────────┘            │
│  │ (启动+定时)    │                                       │
│  └───────────────┘                                       │
│  commands.rs — list/get/copy/delete/clear/settings/stats │
└──────────────────────────────────────────────────────────┘
```

## 4. 剪贴板监听（Windows）

- 后台线程轮询 `GetClipboardSequenceNumber()`（每 300ms），序列号变化即表示剪贴板被更新。
- 相比「消息窗口 + `AddClipboardFormatListener`」，轮询实现更简单、同样可靠，且避免窗口类/消息循环样板代码；300ms 延迟对剪贴板管理器可接受。
- 读取时用重试循环打开剪贴板（`OpenClipboard` 最多重试 10 次、间隔 10ms），规避写入方短暂持锁。
- **去重**：对连续两条相同内容做 FNV-1a 64 位哈希去重，避免噪声。
- **回写自捕获抑制**：`copy_to_clipboard` 时记录写入内容的哈希，监听线程遇到相同哈希即跳过，避免把自己写回的内容再次入库。

## 5. 数据提取（优先级从高到低）

读取时 `OpenClipboard` → 探测可用格式 → 提取 → `CloseClipboard`：

1. **图片**：`PNG`（注册格式）→ `CF_DIBV5` → `CF_DIB` → 资源管理器复制文件（`CF_HDROP`）→ HTML 内嵌 `data:image`；DIB/其他格式经解析转为 PNG 落盘。
2. **富文本**：`CF_HTML` → `CF_RTF`。
3. **纯文本**：`CF_UNICODETEXT` → `CF_TEXT`。

一次剪贴板快照 = 一条记录，同时携带它拥有的所有表示（text/html/rtf/image）；按「最丰富」决定展示类型：`image > html > rtf > text`。非 UTF-8 内容按系统 ANSI 编码解码（`MultiByteToWideChar`），并支持 UTF-16 BOM。

## 6. 数据模型（JSON 文件存储）

- `clips.json`：`Vec<Clip>` 数组，每元素字段：
  - `id`、`kind`（`image`/`html`/`rtf`/`text`）、`text`、`html`、`rtf`、`image_path`
  - `source_app`、`preview`、`char_count`、`byte_size`
  - `created_at`、`expires_at`（Unix 毫秒；`null` = 永久）、`content_hash`
  - `pinned`（置顶，永不过期）、`thumb_base64`（图片缩略图）
- `settings.json`：`retention_hours`、`max_clips`、`theme`
- 图片文件：`<data_dir>/images/<id>_<created_at>.png`
- 持久化采用「写临时文件 + 原子 rename」，崩溃安全。
- 全部记录常驻内存（`RwLock<Vec<Clip>>`），搜索/过滤在内存完成，每次变更整体持久化。

## 7. 保留时间与清理

- 设置项 `retention_hours`：`24 | 72 | 168 | 720 | null`（`null` = 永久，默认 `168` 即 7 天）。
- 写入时 `expires_at = created_at + retention_hours * 3600_000`；永久 → `null`。
- **清理任务**：应用启动时执行一次 + 每 5 分钟执行一次，删除 `expires_at < now` 的记录并删除对应图片文件。
- 修改保留时间时可勾选「应用到已有记录」：对现有记录 `expires_at = now + retention`（从现在重新计时）。
- `max_clips`（默认 500）：超出时裁剪最旧记录。

## 8. 回写剪贴板

- 文本：写 `CF_UNICODETEXT`（带结尾 NUL 的 UTF-16LE）。
- 富文本：写 `CF_HTML`（按规范生成 header + fragment）+ `CF_UNICODETEXT`（纯文本）+ `CF_RTF`。
- 图片：写 `PNG`（注册格式）+ `CF_DIB`（32bpp BI_RGB，由 PNG 解码为 RGBA 再编码）。
- 回写后把该记录时间戳刷新并移到顶部（类似 Windows 剪贴板历史的行为）。

## 9. 命令接口（IPC）

| 命令 | 说明 |
| --- | --- |
| `list_clips { limit, offset, kind, search }` | 分页/过滤/搜索，返回摘要列表 |
| `get_clip { id }` | 返回完整内容（含 HTML/图片 base64） |
| `copy_to_clipboard { id }` | 把记录内容写回剪贴板并置顶 |
| `delete_clip { id }` / `clear_all` | 删除单条 / 清空 |
| `pin_clip { id, pinned }` | 置顶 / 取消置顶（置顶记录永不过期、排最前） |
| `get_settings` / `update_settings` | 读/写设置（保留时间、最大条数、主题） |
| `get_stats` | 记录数、占用空间、图片数 |

## 10. UI 设计（简洁优雅）

- **布局**：顶部工具栏（品牌 + 搜索框 + 类型过滤 + 清空/设置按钮）；左侧列表（时间倒序，类型图标 + 预览 + 相对时间 + 来源应用）；右侧预览区（文本 / 富文本渲染 / 图片预览）。
- **视觉语言**：充足留白、圆角、柔和阴影、克制配色；浅色/深色主题；系统字体栈；细腻的悬浮/选中态与自定义滚动条。
- **交互**：单击预览、双击写回剪贴板、悬浮删除按钮、底部 toast 提示、空状态友好引导。
- **安全**：富文本在 `sandbox` iframe 中渲染，禁止脚本执行。

## 11. Rust 模块划分

```
src-tauri/src/
  main.rs        # 入口
  lib.rs         # App 构建、命令注册、状态注入、监听/清理线程
  clipboard.rs   # 原生 FFI + 剪贴板捕获与回写（text/html/rtf/image）
  dib.rs         # DIB <-> RGBA 转换（支持 1/4/8/16/24/32bpp、BI_RGB/BI_BITFIELDS）
  storage.rs     # JSON 存储、模型、CRUD 与去重
  settings.rs    # 设置读写
  commands.rs    # Tauri commands
```

## 12. 关键依赖

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
image = { version = "0.25", default-features = false, features = ["png"] }
base64 = "0.22"

[devDependencies]
@tauri-apps/cli = "^2"
```

## 13. 里程碑

1. 脚手架：Tauri 2 vanilla 模板，可启动空窗口。
2. 存储层：JSON 存储 + CRUD + 去重。
3. 剪贴板监听与提取：文本/HTML/RTF/图片入库。
4. 清理任务 + 设置（retention / max_clips / theme）。
5. 前端 UI + IPC 联调。
6. 编译验证（`cargo check` / `tauri build`）。
