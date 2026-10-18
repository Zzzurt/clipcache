# ClipCache 界面改版 · 设计稿 v1

> **状态：已落地。** 变体 A（赤陶）已经写进 `src/index.html` / `src/styles.css`，`src/main.js` 改了少量渲染分支（含富文本配色归一化）。
> 这个目录保留完整的决策记录、旧版对照页和可重跑的出图脚本。

## 怎么看

| 文件 | 内容 |
| --- | --- |
| `docs/shots/before-after-light.png` | **浅色前后并排大图**（README 用） |
| `docs/shots/before-after-dark.png` | **深色前后并排大图**（README 用） |
| `docs/shots/light.png` / `dark.png` | README 顶部的浅色 / 深色主界面（真实 exe） |
| `docs/shots/richtext-fix.png` | 富文本配色修复前后对照 |
| `design/shots/demo-modal.png` | 真实 exe 的设置弹窗 |
| `design/shots/before-light.png` / `before-dark.png` | 改版前（`build-before.mjs` 从 git 历史重放） |
| `design/shots/states.png` | 状态四宫格：弹窗 / 图片 / 窄窗 / toast |
| `design/mock/after.html` | 设计稿原型（自包含 CSS），保留三个色系变体 |

`mock/before.html` 与 `mock/live.html` 由脚本生成、不入库（见下方出图流程）。

`live.html` / `after.html` 支持 URL 参数，方便单独看某个状态：

```
live.html?select=3            选中第 3 条（富文本）
live.html?select=1            选中第 1 条（图片）
live.html?theme=dark          深色
live.html?modal=1             设置弹窗
live.html?toast=1             深色 toast + 撤销
（不带参数 = 空态占位）

after.html?variant=neutral    变体 B：中性灰（可叠加 &theme=dark）
after.html?variant=paper      变体 C：纸感（可叠加 &theme=dark）
```

重新出图：

```
cd design
node build-before.mjs                   # git 历史里的旧版 src/ + fixture → mock/before.html
node shot.mjs "mock/before.html?theme=light" shots/before-light.png
node shot.mjs "mock/before.html?theme=dark"  shots/before-dark.png

node build-live.mjs                     # src/ + Tauri 桩 → mock/live.html
node shot.mjs "mock/live.html?select=3" shots/live-light.png
node check.mjs                          # 校验 id / class 覆盖
```

> `build-before.mjs` 用 `git show 384f250:src/index.html` 取旧版，不保留副本文件——仓库里已有那份历史。

> `shot.mjs` 带了 `--virtual-time-budget=8000`。去掉它，2s 轮询和异步 `invoke` 还没跑完就截图，预览区会是空的。这个值最初是 2500，但富文本 iframe 可能要到首次绘制之后才可见——同一个页面用 2500 拍出全白、用 8000 拍出正文，所以调大到 8000 才稳定。**预览区空白时先怀疑截图时机，别改渲染逻辑。**
>
> 想用真实剪贴记录调试配色，设 `CC_HTML_CLIP=<id>`（从 `%APPDATA%\com.clipcache.app\clips.json` 只读取第 id 条注入为 `?select=999` 的行）。它只读不写，也不会把内容写进 `live.html`。

## 驱动真实 exe 截图（CDP）

真实窗口截图不再依赖窗口焦点。给 WebView2 开 DevTools 端口，用协议直接点 DOM：

```
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
$env:CLIPCACHE_DATA_DIR = "$env:TEMP\cc-demo"
start src-tauri\target\release\clipcache.exe

node live.mjs "shots/real-light.png" --id 3
node live.mjs "shots/real-modal.png" --id 3 --modal
```

`live.mjs` 连 `http://127.0.0.1:9222/json/list`，用 `Runtime.evaluate` 派发真实 `mousedown` + `click`，再用 `Page.captureScreenshot` 带 `clip.scale: 2` 出 2 倍图。不抢焦点、不受 z-order 影响，这是当前唯一可靠的交互式截图方式。用 `--id <clip id>` 选行比 `--row N` 稳，滚动后行号会变；端口可用 `CC_CDP_PORT` 覆盖。

> 别用 `getCurrentWindow().setSize()` 改窗口尺寸——Tauri 2 的 ACL 默认不允许 `plugin:window|set_size`，会抛 `not allowed by ACL`。

演示数据由 `make-demo-data.py` 生成（8 条，含置顶富文本、两张渐变图、SQL、URL、命令、部署清单）。

> 缩略图文件名必须是 `thumb_<id>.png`（见 `src-tauri/src/storage.rs` 的 `thumb_path`），写成 `thumb_<原文件名>.png` 会让列表缩略图空白。

旧的 `grab.ps1`（`PrintWindow` + `PW_RENDERFULLCONTENT`）仍然可用，但它只能截图、不能交互，适合不需要选中的场景：

```
powershell -NoProfile -ExecutionPolicy Bypass -File design\grab.ps1 -Row 2 -Out design\shots\x.png
```

> 用 `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` 取窗口矩形，而不是 `GetWindowRect`——后者含不可见的阴影边距，截出来会多一圈黑边。
> 另外注意：`decorations: false` + `shadow: false` 时窗口在屏幕上没有系统标题栏，截图里看到的就是全部内容。


## 三个色系方向

版式、间距、发丝线、图标全部共用同一套，三种变体只换 `:root` 变量与极少量质感规则（`html[data-variant="..."]` 前缀），所以可以随时整组切换。

| | A 赤陶（默认） | B 中性灰 | C 纸感 |
| --- | --- | --- | --- |
| `--bg` / `--surface` | `#f5f4f1` / `#fcfbf9` | `#f4f4f5` / `#fbfbfc` | `#f1ebdf` / `#faf5e9` |
| `--ink` | `#1b1a17` | `#18181a` | `#241f18` |
| `--accent` | `#b4552e` 赤陶橙 | `#34343a` 石墨 | `#96522a` 赭石 |
| 选中态 | 9% 橙底 + 橙竖条 | 6% 石墨底 + **内描边** + 深竖条 | 8.5% 赭石底 + 赭石竖条 |
| 质感 | 无 | 圆角收紧到 6px，logo 去投影 | 5% `feTurbulence` 纸纤维，预览标题转衬线 |
| 深色 `--accent` | `#e08a5b` | `#d8d8de` | `#d99a63` |

最终选了 **A 赤陶**。三个方向可以直接在 `mock/after.html?variant=` 里切换对比（`neutral` / `paper`，可叠加 `&theme=dark`）。

取舍：

- **A 赤陶**：个性最强，一眼记住。风险是橙红在中文语境里容易被读成警示色，所以橙色面积必须压到极小。
- **B 中性灰**：最克制，零色相，彻底消失在系统里。代价是层级全靠间距，稍一放松就发灰发平——这也是它需要额外加内描边的原因。
- **C 纸感**：气质最独特，适合"读"占比高的场景（富文本、长文预览）。风险是纸底吃掉对比度余量，深色下要盯 AA。

> 注：纸纤维用 `mix-blend-mode: multiply`（深色下 `screen`）压在最上层，`pointer-events: none`，不影响交互。落地时如果担心合成层开销，可以只保留在预览区背景上。

## 设计语言：Quiet Paper

剪贴板管理器是「扫一眼就走」的工具，界面应该退后一步。三层决定：

1. **去彩色化。** 旧版蓝色同时出现在主按钮、选中底、焦点环上，互相抢。新版全站只有一个彩色——赤陶橙 `#b4552e`，只用于选中态、焦点环、置顶标记和富文本 bullet；主按钮反过来用油墨黑 `#1b1a17`。
2. **去卡片化。** 旧版靠卡片 + 阴影分层，在 1000px 的小窗里显得拥挤。新版只用 `1px` 发丝线：列表行之间是缩进到 `left: 61px` 的短线（让缩略图一列贯通），区域之间是全宽线，整屏只有弹窗用一处投影。
3. **排版承担层级。** 层级靠字重、字距、行高和留白，而不是颜色和装饰。

## 关键改动

| 位置 | 旧 | 新 | 为什么 |
| --- | --- | --- | --- |
| 顶栏高度 | 12px padding，自适应 | 固定 `56px` | 与过滤器下划线对齐，成为一条完整的分隔线 |
| 类型过滤器 | 胶囊按钮组 | 贴底边的文字 tab + 2px 橙下边线 | 胶囊是"控件"，下划线 tab 是"位置"，后者更轻，省掉一整排边框 |
| 列表行 | flex + 圆角 10px + hover 底色 | 三列 grid（34px / 1fr / 20px）+ 缩进发丝线 | 缩略图、文字、删除三列严格对齐，行间线不穿过图标 |
| 选中态 | 整行 `--accent-soft` 色块 | 9% 橙底 + 左侧 2px 竖条 + 标题转 `#8f3f20` | 列表信息已经很密，实心色块过载；竖条足够定位 |
| 置顶标记 | 药丸徽章（带边框） | 5px 圆点 + 两个字 | 降低一个装饰层级 |
| 来源应用 | 带边框的小 tag | 纯文字，用 2px 圆点分隔 | 同上 |
| 预览正文 | 铺满整个画布 | 收进 `max-width: 660px` 居中的 `.sheet`，上下 34 / 56px | 长文本行宽失控；阅读栏让视线不用横跨整个窗口 |
| 正文排版 | 14px / 1.9，浏览器默认 bullet | 13.5px / 1.65，7px 橙圆角方块 bullet | 更紧的行高 + 更克制的符号 |
| 主按钮 | 蓝色实心 `#4f6ef7` | 油墨实心 `#1b1a17` | 把"强调"留给橙，主操作靠明度对比 |
| 状态栏 | 左侧统计 + 右侧一句提示 | 统计用圆点分隔 + 右侧三组键盘提示 | 「选中—回车—走人」是核心流程，快捷键不该只写在 README |
| 深色 | 冷灰 `#131417` 三级 | 暖黑 `#100f0e / #161513 / #262420`，强调提到 `#e08a5b` | 冷灰配蓝在深底上发灰；暖黑更耐看，橙的对比也够 |

## 设计令牌

```css
/* 浅色 */
--bg:        #f5f4f1;   /* 纸底 */
--surface:   #fcfbf9;   /* 预览区 */
--chrome:    #f8f7f4;   /* 顶栏 / 状态栏 */
--field:     #f0eee9;   /* 输入框 */
--surface-2: #edeae4;   /* 图标底 / hover */

--ink:       #1b1a17;   /* 正文 / 主按钮 */
--ink-soft:  #3f3d37;
--muted:     #79756c;
--faint:     #a5a096;

--line:       #e4e1da;  /* 发丝线 */
--line-strong:#d5d1c8;

--accent:     #b4552e;  /* 赤陶橙，全站唯一彩色 */
--accent-ink: #8f3f20;
--accent-soft: rgba(180, 85, 46, 0.09);
--accent-ring: rgba(180, 85, 46, 0.16);

/* 深色 */
--bg: #100f0e; --surface: #161513; --chrome: #141312;
--field: #1c1a18; --surface-2: #211f1c;
--ink: #edeae2; --ink-soft: #cdc8bd; --muted: #8f8a80; --faint: #6b675f;
--line: #262420; --line-strong: #35312b;
--accent: #e08a5b; --accent-ink: #f0a77e;
```

字体不变，仍是系统栈；唯一新增的是等宽字体栈 `--mono`（`Cascadia Code` 优先），给纯文本预览和数据目录用。

**图标尺寸**：顶栏图标一律按「墨迹尺寸」对齐，而不是按 SVG 视区对齐。垃圾桶路径只占 18/24 视区，齿轮撑满 24/24，所以齿轮单独收到 `11px` 并加线宽到 `2.4`——两者实测墨迹都是 11×11 CSS px。这是 `src/index.html` 里原来 16px 齿轮和 16px 垃圾桶看起来"齿轮更大"的根因。

## 实施时的落点

已落地的改动范围：

- `src/index.html`：整文件重写，**所有元素 id 一个没动**。`.filters` 保留 `data-kind`；列表行从 `flex` 改成三列 `grid`；`.brand-dot` → `.mark` + `.wordmark`；状态栏右侧新增 `.hints`（纯 HTML，无 JS）；设置弹窗拆出 `.modal-head` / `.modal-body`，新增 `#set-close`。
- `src/styles.css`：整文件重写（740 → 约 900 行），令牌、间距、发丝线、图标尺寸全部按本文档执行。`:root` 里保留了 `--text`（`#1b1a17`）作为 `--ink` 的别名——`main.js` 会用 `getPropertyValue("--text")` 给富文本沙箱 iframe 注入前景色，删掉它会白底黑字失效。
- `src/main.js`：三处小改（`$("pv-type").dataset.kind = d.kind`、`icon.className` 追加 `is-html`、`$("set-close")` 绑 `closeSettings`）之外，另加了富文本配色的 `sanitiseRichText()`（见下节）。
- `docs/shots/light.png` / `dark.png`：README 顶部用的两张主界面截图，已取代旧的 `docs/screenshot.png`（后者随这次清理删除）。

## 富文本配色归一化

复制来的 HTML 带着来源页面的调色板。从深色站点复制的文字留着 `color: rgb(231,233,235)` 这样的近白声明，落在浅色预览底上几乎看不见；深色模式下反过来。

处理方式（`src/main.js` 的 `sanitiseRichText`）：

1. 用 `DOMParser` 建惰性文档（不执行脚本、不加载图片），按文档序 DFS，用两个 Map 记录每个元素的**实际底色与实际墨色**。
2. `background-color` / `background`：解析不出颜色或 `alpha < 0.5` 时**原样保留**——`background: transparent` 是在清掉浏览器默认按钮底，删掉它灰色方块就冒出来了。近黑 / 近白（亮度 `< 0.1` 或 `> 0.9`）的画布填充丢弃；中间调与图片保留。自定义元素（`<bili-comment-thread-renderer>` 之类）默认是 inline，这类画布本来就没盖住块级子元素，留着只是伪造一个不存在的深色底。
3. `color`：对文字实际所在底色算 WCAG 对比度，低于 3 才换成主题墨色（深底换 `#f4f2ed`，浅底换 `--text`）。链接、引用、高亮因此都保住了。
4. 最后删掉 `style / link / script / meta`——来源页的版式会跟预览打架。

这个逻辑用真实 `clips.json` 里的 B 站评论区整段（36 KB、43 处颜色声明）验证过浅深两色。


## 还没做的

- 富文本预览的沙箱渲染逻辑、图片预览的缩放/适应模式没有动，只是套了外层排版。
- 列表虚拟滚动、多选、批量操作属于功能改动，不在这一版范围内。
- 动效只有 hover / focus 的 120–160ms 过渡，没有加任何入场动画。
