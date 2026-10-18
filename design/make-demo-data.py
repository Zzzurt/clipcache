# Builds a curated, README-quality data dir so the screenshots show the app's
# real UI with representative content instead of whatever was on the clipboard.
# Usage: python make-demo-data.py <out-dir>
import json
import os
import sys
import time

from PIL import Image, ImageDraw, ImageFilter

OUT = sys.argv[1] if len(sys.argv) > 1 else "demo"
IMAGES = os.path.join(OUT, "images")
os.makedirs(IMAGES, exist_ok=True)

NOW = int(time.time() * 1000)
MIN = 60 * 1000
HOUR = 60 * MIN


def gradient(w, h, stops, angle=135):
    """Linear gradient over a diagonal axis — enough to read as a real photo
    thumbnail at 34px without shipping binary fixtures."""
    img = Image.new("RGB", (w, h))
    px = img.load()
    import math

    rad = math.radians(angle)
    dx, dy = math.cos(rad), math.sin(rad)
    span = abs(dx) * w + abs(dy) * h
    for y in range(h):
        for x in range(w):
            t = ((x * dx + y * dy) / span) if span else 0
            t = min(max(t, 0), 1)
            seg = t * (len(stops) - 1)
            i = min(int(seg), len(stops) - 2)
            f = seg - i
            a, b = stops[i], stops[i + 1]
            px[x, y] = tuple(round(a[k] + (b[k] - a[k]) * f) for k in range(3))
    return img


def soft_photo(seed_hue):
    w = h = 640
    base = gradient(
        w,
        h,
        [(232, 214, 196), (198, 168, 142), (120, 108, 100), (58, 54, 52)],
        angle=125,
    ).filter(ImageFilter.GaussianBlur(6))
    d = ImageDraw.Draw(base, "RGBA")
    if seed_hue == "warm":
        d.ellipse((300, 250, 700, 650), fill=(46, 40, 36, 90))
        d.arc((60, 150, 520, 620), 200, 330, fill=(246, 236, 224, 150), width=3)
    else:
        d.ellipse((120, 200, 520, 600), fill=(28, 30, 34, 80))
        d.line((40, 470, 300, 300, 430, 380, 620, 210), fill=(240, 238, 232, 170), width=3)
    return base.filter(ImageFilter.GaussianBlur(1))


def save(im, cid):
    """Image files are `<id>_<ms>.png`, thumbnails are `thumb_<id>.png`
    (see Store::thumb_path in src-tauri/src/storage.rs)."""
    name = "%d_%d.png" % (cid, NOW)
    im.save(os.path.join(IMAGES, name), "PNG")
    thumb = im.copy()
    thumb.thumbnail((96, 96), Image.LANCZOS)
    thumb.save(os.path.join(IMAGES, "thumb_%d.png" % cid), "PNG")
    return name


DEPLOY_HTML = """<h1>部署清单</h1>
<ul>
  <li>执行数据库迁移</li>
  <li>更新环境变量</li>
  <li>重启服务并验证健康检查</li>
</ul>"""


def long_html():
    body = """
    <h1>接口联调记录</h1>
    <p>今天把导出任务的三个接口对完了，记一下结论，免得下次再翻聊天记录。</p>
    <h2>分页</h2>
    <p>游标分页的 <code>next_cursor</code> 在最后一页返回空字符串而不是 null，
    前端判断要用 falsy 而不是 <code>=== null</code>。这个坑上次也踩过。</p>
    <h2>错误码</h2>
    <ul>
      <li>429 要读 <code>Retry-After</code> 头，不要固定退避 2 秒</li>
      <li>409 表示幂等键冲突，可以直接当作成功返回</li>
      <li>500 一律重试两次，第三次直接抛给用户</li>
    </ul>
    <h2>待确认</h2>
    <ol>
      <li>导出文件的 retention 由谁负责清理</li>
      <li>大文件是否走分片，还是直接流式返回</li>
    </ol>
    <p>明天上午十点跟后端再对一次。</p>
    """
    return body


clips = []


def add(kind, preview, *, text=None, html=None, image=None, app=None,
        ago=0, byte_size=0, pinned=False, char_count=0):
    cid = 300 + len(clips) + 1
    created = NOW - ago
    entry = {
        "id": cid,
        "kind": kind,
        "text": text,
        "html": html,
        "rtf": None,
        "image_path": image,
        "source_app": app,
        "preview": preview,
        "char_count": char_count or (len(text) if text else 0),
        "byte_size": byte_size,
        "created_at": created,
        "expires_at": None if pinned else created + 72 * HOUR,
        "content_hash": "demo-%d-%d" % (cid, created),
        "pinned": pinned,
        "has_thumb": image is not None,
    }
    clips.append(entry)


img_a = save(soft_photo("warm"), 302)
img_b = save(soft_photo("cool"), 304)

add("html", "接口联调记录 今天把导出任务的三个接口对完了，记一下结论…",
    html=long_html(), app="Notion", ago=4 * MIN, byte_size=1842, pinned=True)
add("image", "图片 640×400", image=img_a, app="Photoshop", ago=27 * MIN, byte_size=1212000)
add("text", "SELECT id, title, created_at FROM articles WHERE status = 'draft' …",
    text="SELECT id, title, created_at FROM articles WHERE status = 'draft' ORDER BY created_at DESC LIMIT 20;",
    app="VS Code", ago=52 * MIN, byte_size=108)
add("image", "图片 640×400", image=img_b, app="Figma", ago=3 * HOUR, byte_size=880400)
add("text", "https://github.com/Zzzurt/clipcache/pulls",
    text="https://github.com/Zzzurt/clipcache/pulls", app="Chrome", ago=4 * HOUR, byte_size=48)
add("text", "cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings",
    text="cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings",
    app="Windows Terminal", ago=5 * HOUR, byte_size=79)
add("html", "部署清单", html=DEPLOY_HTML, app="Notion", ago=8 * HOUR, byte_size=214)
add("text", "记得把 keys 从 .env 挪到系统环境变量，再删除本地文件。",
    text="记得把 keys 从 .env 挪到系统环境变量，再删除本地文件。", app="WeChat", ago=26 * HOUR, byte_size=96)

with open(os.path.join(OUT, "clips.json"), "w", encoding="utf-8") as f:
    json.dump(clips, f, ensure_ascii=False, indent=2)

light = {"retention_hours": 72, "max_clips": 100, "theme": "light"}
dark = dict(light, theme="dark")
for name, cfg in (("settings.json", light), ("settings.dark.json", dark)):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)

print("wrote %d clips -> %s" % (len(clips), OUT))
