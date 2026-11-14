"""Pixel regression: changing home groups must preserve the heading's origin."""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_directory", type=Path)
    parser.add_argument("output_path", type=Path)
    args = parser.parse_args()
    checks = []
    for renderer in ("software", "femtovg"):
        for theme in ("light", "dark"):
            directory = args.capture_directory / renderer / "images"
            with Image.open(directory / f"{theme}-home.png") as baseline:
                reference = baseline.convert("RGB").crop((16, 80, 320, 165))
            for scene in ("home-empty-pinned", "home-empty-frequent", "home-two-rows"):
                with Image.open(directory / f"{theme}-{scene}.png") as candidate:
                    if candidate.size != (640, 660):
                        raise ValueError("Regression uses 640×660 component captures only")
                    difference = ImageChops.difference(reference, candidate.convert("RGB").crop((16, 80, 320, 165)))
                    passed = difference.getbbox() is None
                    checks.append({"Renderer": renderer, "Theme": theme, "Scene": scene, "HeadingOriginPreserved": passed})
    report = {"Passed": all(row["HeadingOriginPreserved"] for row in checks), "Checks": checks,
              "Scope": "Unscaled component pixels; does not establish native Wails or physical DPI geometric tolerance."}
    if args.output_path.exists():
        raise ValueError("Use a fresh output path")
    args.output_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if not report["Passed"]:
        raise ValueError("Changing home groups shifted the heading")
    print(f"Passed {len(checks)} independent heading-origin pixel regressions.")


if __name__ == "__main__":
    main()
