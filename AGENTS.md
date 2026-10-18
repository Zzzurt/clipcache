# AGENTS.md — Clip-Cache

Windows 剪贴板管理器。Tauri 2 + 原生 HTML/CSS/JS，无框架、无构建步骤。

## 提交/推送前的强制检查

**在 `git commit` 与 `git push` 之前，必须先做一次「多余文件」检查。** 顺序：

1. `git status --short` —— 逐个确认 `??`（未跟踪）与 `M`（已修改）的条目。
   任何一个都不允许"顺手带上"，必须能说出它属于本次改动。
2. 对每个未跟踪项判断是**要入库**还是**属于 .gitignore**：
   - 能由脚本/构建重新生成的 → 加入 `.gitignore`，不入库
   - 本地环境产物（缓存、profile、日志、临时目录）→ 加入 `.gitignore`
   - 大体积二进制、截图副本、重复文件 → 先确认是否真被引用；重复的删掉，只留一份
   - 只有源码、测试、文档、以及**文档真正引用的**资产才入库
3. `git check-ignore -v <path>` 验证新规则确实生效；再看 `git status --short` 是否已干净。
4. 用 `git ls-tree -r -l HEAD` 或按目录统计体积，确认没有意外膨胀（当前基线：约 47 个文件 / 3.1 MB，其中 `docs/shots/` 占 2.6 MB）。
5. 推送前跑一遍真正的门禁（见下）。**任何一步红都不许推。**

只在 `.gitignore` 里加规则不够——如果文件此前已被跟踪，必须 `git rm --cached` 才会真正脱管。

## 验证门禁

改动 `src/` 后至少跑：

```powershell
node scripts/check.mjs      # DOM id 与 main.js 的一致性、CSS 未定义 class
node scripts/linkcheck.mjs  # README 里的相对链接是否都存在
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

CI（`.github/workflows/ci.yml`）跑的就是这三条 cargo 命令，本地红 == CI 红。

## 已入库 / 不入库的边界

- **入库**：`src/`、`src-tauri/src`、`scripts/`（`check.mjs`、`linkcheck.mjs`、`make-icon.py`）、`docs/shots/`（README 引用的截图）。
- **不入库**：整个 `design/`（本地设计工作区：决策记录、设计稿原型、截图工具、前后对照图）、`/docs/0*.md` 与 `/docs/README.md`（本地设计笔记）、各类缓存与 profile。它们留在磁盘上，只是不进 git。
- 新增文件前先问：它能不能由仓库里已有的东西重新算出来？能 → 别提交。另外问一句：它只对我或 AI 有用，还是对任何克隆仓库的人都有用？前者放 `design/`，后者才入库。

## 构建

```powershell
npm run tauri build
```

- 产物时间戳与日志 `Finished 2 bundles at:` 才是成功依据；**pwsh 管道（`Select-Object`）会让后台 job 误报 exit code 1**。
- 构建前先停掉正在运行的 `clipcache` 进程，否则 release exe 被占用会报 `os error 5 拒绝访问`。
- 前端资源编译进二进制（`tauri.conf.json` 的 `frontendDist: "../src"`），改了 `src/` 必须重新构建才能在真实窗口生效。

## 约定

- 版本号三处同步：`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`package.json`。
- commit 用中文 conventional commit，正文按主题分小节。
- 不要动用户真实数据 `%APPDATA%\com.clipcache.app\`；截图与演示一律用 `CLIPCACHE_DATA_DIR` 指向临时目录隔离。
