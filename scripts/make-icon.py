# 生成 app 图标：形状与顶栏 `.mark` / 空态 glyph 完全同源。
#
# 设计约束（对应 design/README.md 的改版说明；design/ 是本地工作区、不入库）：
#   - 形状 = 顶栏 24×24 glyph 本身：前面一张便签 rect(8,3,12,14,r2.5)，后面一张
#     纸只露左、下两条边（M16 20.5H6.5A2.5 2.5 0 0 1 4 18V7）。不改成「两张完整
#     便签叠放」——那会让图标和窗口里的 mark 长得不一样；
#   - 配色 = 顶栏 mark 徽标：--ink #1b1a17 底 + --surface 色线条。不用赤陶橙：
#     橙在界面里只表示选中/焦点/置顶，做成图标底色会让「全站只保留一种彩色用于
#     强调」这条约束失效；
#   - 小尺寸单独调：glyph 占比、线宽、两线间距都按目标像素给下限。16px 时 1 个
#     glyph 单位只有 0.62px，线宽 2 单位 = 1.2px、两线间距 2 单位也是 1.2px，
#     直接等比缩会糊成一团黑块。
#
# 用法：python scripts/make-icon.py            # 写 src-tauri/icons/*
#       python scripts/make-icon.py --sheet    # 只重出 design/shots/icon-sheet.png（本地对照图）
from __future__ import annotations

import math
import struct
import sys
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "src-tauri" / "icons"
# 尺寸对照图只进本地设计工作区（design/ 不入库），别的地方不需要它
SHOTS = ROOT / "design" / "shots"

INK = (27, 26, 23, 255)       # --ink     #1b1a17
PAPER = (252, 251, 249, 255)  # --surface #fcfbf9

# glyph 在 24 单位坐标系里的包围盒：x∈[4,20]、y∈[3,21]，中心 (12,12)
FRONT = (8.0, 3.0, 20.0, 17.0)
RADIUS = 2.5
GLYPH_H = 18.0
CX, CY = 12.0, 12.0

# size -> (glyph 高度占画布比例, 线宽下限 px, 两条线间距下限 px)
# 大尺寸按 glyph 原有比例（线宽 2 单位、间距 2 单位）；小尺寸用 px 下限兜底：
# 等比缩到 16px 时 2 单位只剩 1.1px、两条线之间不足 1px，会糊成一团黑块。
TUNING = [
    (16, 0.70, 1.8, 1.5),
    (20, 0.70, 1.9, 1.6),
    (24, 0.70, 2.2, 2.0),
    (32, 0.70, 2.6, 2.4),
    (48, 0.69, 3.6, 3.2),
    (64, 0.68, 4.6, 4.2),
    (128, 0.68, 8.8, 8.4),
    (256, 0.67, 17.6, 16.8),
    (512, 0.66, 35.2, 33.6),
    (1024, 0.66, 70.4, 67.2),
]

ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]

STROKE_UNITS = 2.0  # 与顶栏 glyph 的 stroke-width 一致
GAP_UNITS = 2.0     # 前面那张便签与后面那条边的净空


def tuning(size: int) -> tuple[float, float, float]:
    gh, sw, gap = TUNING[0][1], TUNING[0][2], TUNING[0][3]
    for k, kh, ks, kg in TUNING:
        if size >= k:
            gh, sw, gap = kh, ks, kg
    return gh, sw, gap


def metrics(size: int) -> tuple[float, float, float]:
    """→ (24 单位到 px 的比例, 线宽 px, 间距 px)。"""
    gh, min_stroke, min_gap = tuning(size)
    scale = (gh * size) / GLYPH_H
    unit = scale  # 1 个 glyph 单位 = scale px（画布坐标）
    return scale, max(STROKE_UNITS * unit, min_stroke), max(GAP_UNITS * unit, min_gap)


def corner_ratio(size: int) -> float:
    # Windows 应用图标的习惯：小尺寸略方，大尺寸略圆。
    if size <= 24:
        return 0.17
    if size <= 64:
        return 0.20
    return 0.22


def stamp_polyline(draw: ImageDraw.ImageDraw, pts, width: float, fill) -> None:
    """画一条圆头粗折线。

    Pillow 的 joint="curve" 在圆弧的密集采样点上会漏缝（线宽越接近半径越明显），
    所以除了 line() 之外，再在每个采样点补一个圆把接缝盖住。
    """
    draw.line(pts, fill=fill, width=max(1, round(width)), joint="curve")
    r = width / 2
    for x, y in pts:
        draw.ellipse((x - r, y - r, x + r, y + r), fill=fill)


def render(size: int) -> Image.Image:
    """按目标像素尺寸直接排版：线宽与间距以 px 为准，再乘超采样倍数。"""
    ss = 16 if size <= 32 else (8 if size <= 128 else 4)
    w = size * ss
    img = Image.new("RGBA", (w, w), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # ---------- 底板 ----------
    d.rounded_rectangle((0, 0, w - 1, w - 1), radius=corner_ratio(size) * w, fill=INK)

    # ---------- glyph ----------
    gh, min_stroke, min_gap = tuning(size)
    scale = (gh * w) / GLYPH_H          # 24 单位 → 画布像素
    t = max(STROKE_UNITS * scale, min_stroke * ss)
    g = max(GAP_UNITS * scale, min_gap * ss)
    r = RADIUS * scale

    def P(x, y):
        return ((x - CX) * scale + w / 2, (y - CY) * scale + w / 2)

    def box(rect, grow=0.0):
        x0, y0, x1, y1 = rect
        return (
            (x0 - CX) * scale + w / 2 - grow,
            (y0 - CY) * scale + w / 2 - grow,
            (x1 - CX) * scale + w / 2 + grow,
            (y1 - CY) * scale + w / 2 + grow,
        )

    # 后面那张纸：M16 20.5 → H6.5 → 圆弧到 (4,18) → V7，圆头圆角。
    # 圆弧圆心 (6.5,18)、半径 2.5：起点 90° 处 (6.5,20.5)，扫到 180° 处 (4,18)。
    pts = [P(16, 20.5), P(6.5, 20.5)]
    acx, acy = P(6.5, 18)
    steps = max(12, int(r / (1.5 * ss)))
    for i in range(1, steps + 1):
        a = math.radians(90 + 90 * i / steps)
        pts.append((acx + r * math.cos(a), acy + r * math.sin(a)))
    pts.append(P(4, 7))
    stamp_polyline(d, pts, t, PAPER)

    # 前面那张便签：先用墨色铺一圈把两条线隔开，再描自己的边
    d.rounded_rectangle(box(FRONT, g + t / 2), radius=r + g + t / 2, fill=INK)
    d.rounded_rectangle(box(FRONT, t / 2), radius=r + t / 2, fill=PAPER)
    d.rounded_rectangle(box(FRONT, -t / 2), radius=max(0.0, r - t / 2), fill=INK)

    return img.resize((size, size), Image.LANCZOS)


# ---------------------------------------------------------------- 输出

def png_bytes(img: Image.Image) -> bytes:
    buf = BytesIO()
    img.save(buf, format="PNG", optimize=True)
    return buf.getvalue()


def write_ico(path: Path, sizes: list[int]) -> None:
    """手写 .ico：每档放自己渲染的 PNG。

    Pillow 的 save(format="ico", sizes=...) 从最大图往下缩，小尺寸会糊；
    这里自己拼 ICONDIR + PNG 条目（Vista 起支持 PNG 压缩条目）。
    """
    entries = [(s, png_bytes(render(s))) for s in sizes]
    directory = b""
    body = b""
    offset = 6 + 16 * len(entries)
    for s, data in entries:
        dim = 0 if s >= 256 else s
        directory += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        body += data
        offset += len(data)
    path.write_bytes(struct.pack("<HHH", 0, 1, len(entries)) + directory + body)


def write_icns(path: Path) -> None:
    """macOS 图标。

    Pillow 的 ICNS 写入只支持一种固定结构，而且 macOS 端 Tauri 需要 1024 的
    ic10；这里仍然交给 Pillow，但显式补上 1024，并保留其它档位。
    Windows 是主目标，这份 icns 只要能被 macOS 正确解析即可。
    """
    frames = [render(s) for s in (16, 32, 64, 128, 256, 512)]
    base = render(1024)
    base.save(path, format="ICNS", append_images=frames)


def write_sheet() -> None:
    """设计对照页：各尺寸实际像素 + 浅底/深底小尺寸色带。"""
    SHOTS.mkdir(parents=True, exist_ok=True)
    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    pad, gap, cap, cell = 32, 40, 30, 256
    width = pad * 2 + len(sizes) * cell + (len(sizes) - 1) * gap
    height = pad * 2 + cell + cap + 108
    canvas = Image.new("RGBA", (width, height), (245, 244, 241, 255))
    d = ImageDraw.Draw(canvas)

    x = pad
    for s in sizes:
        art = render(s)
        canvas.alpha_composite(art, (x + (cell - s) // 2, pad + (cell - s) // 2))
        label = f"{s}px"
        d.text((x + cell // 2 - d.textlength(label) / 2, pad + cell + 8),
               label, fill=(121, 117, 108, 255))
        x += cell + gap

    y = pad + cell + cap + 4
    bar_w = (width - pad * 2 - gap) // 2
    for i, bg in enumerate(((252, 251, 249, 255), (16, 15, 14, 255))):
        bx = pad + i * (bar_w + gap)
        d.rounded_rectangle((bx, y, bx + bar_w, y + 88), radius=10, fill=bg,
                            outline=(228, 225, 218, 255))
        cx = bx + 26
        for s in (16, 20, 24, 32, 48):
            canvas.alpha_composite(render(s), (cx, y + (88 - s) // 2))
            cx += s + 24

    canvas.convert("RGB").save(SHOTS / "icon-sheet.png", optimize=True)


def main() -> None:
    if "--sheet" in sys.argv:
        write_sheet()
        print("wrote design/shots/icon-sheet.png")
        return

    ICONS.mkdir(parents=True, exist_ok=True)
    render(512).save(ICONS / "icon.png", optimize=True)
    render(32).save(ICONS / "32x32.png", optimize=True)
    render(128).save(ICONS / "128x128.png", optimize=True)
    render(256).save(ICONS / "128x128@2x.png", optimize=True)
    write_ico(ICONS / "icon.ico", ICO_SIZES)
    write_icns(ICONS / "icon.icns")
    # Windows 磁贴 / Store 用图（沿用 tauri icon 命令产出的文件名）
    for s in (30, 44, 71, 89, 107, 142, 150, 284, 310):
        render(s).save(ICONS / f"Square{s}x{s}Logo.png", optimize=True)
    render(50).save(ICONS / "StoreLogo.png", optimize=True)
    write_sheet()
    print("wrote src-tauri/icons/: icon.png, 32x32.png, 128x128.png, 128x128@2x.png,")
    print("  icon.ico (16/20/24/32/40/48/64/128/256), icon.icns, Square*Logo.png, StoreLogo.png")
    print("wrote design/shots/icon-sheet.png")


if __name__ == "__main__":
    main()
