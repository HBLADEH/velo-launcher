"""Publish synthetic Slint captures and descriptive pixel differences (Pillow).

No rescaling is applied to original PNGs. Contact sheets are review thumbnails;
pixel differences do not establish geometric, animation or presentation parity.
"""
import argparse
import hashlib
import html
import json
import platform
import shutil
from pathlib import Path

import PIL
from PIL import Image, ImageChops, ImageDraw, ImageStat


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    manifests = {
        renderer: json.loads((args.capture_directory / renderer / "capture.json").read_text(encoding="utf-8-sig"))
        for renderer in ("software", "femtovg")
    }
    software, femtovg = manifests.values()
    if software["Sources"] != femtovg["Sources"] or not all(m["ReducedMotion"] for m in manifests.values()):
        raise ValueError("Renderer sources or reduced-motion conditions differ")
    names = sorted(image["Name"] for image in software["Images"])
    expected = sorted(f"{theme}-{scene}.png" for theme in ("light", "dark") for scene in software["ExpectedScenes"])
    if names != expected or len(names) != len(set(names)) or names != sorted(image["Name"] for image in femtovg["Images"]) or software["ExpectedScenes"] != femtovg["ExpectedScenes"]:
        raise ValueError("Static scene inventory differs between the source manifest and renderers")
    images = {}
    for renderer, manifest in manifests.items():
        for entry in manifest["Images"]:
            name = entry["Name"]
            if Path(name).name != name or not name.endswith(".png"):
                raise ValueError("Unexpected image name")
            path = args.capture_directory / renderer / "images" / name
            if sha256(path) != entry["SHA256"]:
                raise ValueError(f"Capture hash mismatch: {renderer}/{name}")
            with Image.open(path) as image:
                image.load()
                rgb = image.convert("RGB")
                if max(ImageStat.Stat(rgb).stddev) == 0:
                    raise ValueError(f"Blank capture: {renderer}/{name}")
                images[renderer, name] = rgb
    # Validate inputs before publishing; refuse to overwrite previous evidence.
    args.output_directory.mkdir()
    for renderer, manifest in manifests.items():
        destination = args.output_directory / renderer
        destination.mkdir()
        (destination / "capture.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        for name in names:
            shutil.copyfile(args.capture_directory / renderer / "images" / name, destination / name)
    comparisons = []
    for name in names:
        left, right = images["software", name], images["femtovg", name]
        if left.size != right.size:
            raise ValueError(f"Different pixel dimensions: {name}")
        difference = ImageChops.difference(left, right)
        channel_mean = ImageStat.Stat(difference).mean
        red, green, blue = difference.split()
        nonzero_channel = ImageChops.lighter(ImageChops.lighter(red, green), blue)
        changed = left.width * left.height - nonzero_channel.histogram()[0]
        comparisons.append({
            "Scene": name, "Width": left.width, "Height": left.height,
            "DifferentPixels": changed,
            "DifferentPixelFraction": changed / (left.width * left.height),
            "MeanAbsoluteRGBChannelDifference": sum(channel_mean) / 3,
        })
    for theme in ("light", "dark"):
        theme_names = [name for name in names if name.startswith(theme + "-")]
        sheet = Image.new("RGB", (1260, ((len(theme_names) + 2) // 3) * 240), "#e6e9ef")
        draw = ImageDraw.Draw(sheet)
        for index, name in enumerate(theme_names):
            for renderer_index, renderer in enumerate(manifests):
                column, row = (index % 3) * 2 + renderer_index, index // 3
                x, y = column * 210, row * 240
                draw.text((x + 5, y + 5), renderer + " / " + name.removesuffix(".png"), fill="black")
                thumb = images[renderer, name].copy()
                thumb.thumbnail((200, 210), Image.Resampling.LANCZOS)
                sheet.paste(thumb, (x + 5, y + 25))
        sheet.save(args.output_directory / f"contact-{theme}.png")
    report = {
        "Python": platform.python_version(), "Pillow": PIL.__version__,
        "SummarizerSHA256": sha256(Path(__file__)),
        "SourceMatched": True, "ScenePairs": len(comparisons), "OriginalPNGs": len(images),
        "SyntheticStates": True, "ReducedMotion": True,
        "PhysicalDPIValidated": False, "PhysicalIMEValidated": False,
        "PresentedFramesVerified": False, "CaptureScope": "ComponentSnapshotsOnly",
        "GeometryToleranceVerified": False,
        "DifferenceScope": "Descriptive RGB differences only. Includes text antialiasing, renderer label and caret phase; no pass/fail threshold or geometric tolerance is inferred.",
        "Comparisons": comparisons,
    }
    (args.output_directory / "comparison.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    figures = []
    for name in names:
        figures.append(f"<h2>{html.escape(name)}</h2><div class='pair'>" + "".join(
            f"<figure><figcaption>{renderer}</figcaption><a href='{renderer}/{name}'><img loading='lazy' src='{renderer}/{name}'></a></figure>"
            for renderer in manifests
        ) + "</div>")
    gallery = """<!doctype html><html lang="zh-CN"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>RS-13 static renderer review</title>
<style>body{font:16px system-ui;margin:32px;background:#eef1f6;color:#192b43}h2{font-size:18px;margin-top:32px}.pair{display:flex;gap:20px;align-items:flex-start}figure{margin:0;width:50%;max-width:640px}img{max-width:100%;height:auto;border:1px solid #bcc8d7}figcaption{padding:8px}p{max-width:1000px}</style>
<h1>RS-13 静态截图审查</h1><p>两种 Slint 渲染器、同源码合成数据、减少动画开启。点击图片查看原始像素。
本页仅覆盖组件快照，不包含动画录像、物理 IME/DPI 或显示器帧验收。RGB 差异不等于布局误差。完整状态见 comparison.json。</p>
""" + "\n".join(figures) + "</html>\n"
    (args.output_directory / "gallery.html").write_text(gallery, encoding="utf-8")
    print(f"Published {len(images)} original PNGs, {len(comparisons)} scene pairs; scope is component snapshots only.")


if __name__ == "__main__":
    main()
