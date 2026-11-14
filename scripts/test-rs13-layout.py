"""Compare Slint component bounds with independently captured Wails DOM bounds.

The reference is the unchanged Wails frontend at a 640x660 CSS viewport. Native
pixel/DPI validation is separate; this is not a font rasterization comparison.
"""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError("Use a fresh output path")
    references = json.loads(args.reference.read_text(encoding="utf-8-sig"))
    captures = json.loads(args.capture.read_text(encoding="utf-8-sig"))
    checks = []
    for capture in captures:
        scene = capture["scene"]
        if scene.endswith("-bottom"):
            continue
        tab = int(scene.rsplit("-", 1)[1])
        elements = references[tab]["elements"]
        expected = {
            "header": next(e for e in elements if e["class"].startswith("settings-header")),
            "body": next(e for e in elements if e["class"] == "settings-body"),
            "cancel": next(e for e in elements if e["tag"] == "BUTTON" and e["text"] == "取消"),
            "save": next(e for e in elements if e["tag"] == "BUTTON" and e["text"] == "保存设置"),
        }
        if tab in (0, 3):
            expected["maximum" if tab == 0 else "weight"] = next(e for e in elements if e["tag"] == "INPUT" and e.get("type") == "number")
        if tab == 2:
            expected.update(zip(("theme", "layout"), (e for e in elements if e["class"] == "select-trigger")))
        if tab == 4:
            expected["directories"] = next(e for e in elements if e["tag"] == "TEXTAREA")
        actual = {e["name"]: e for e in capture["bounds"]}
        for name, reference in expected.items():
            delta = {k: abs(actual[name][k] - reference[k]) for k in ("x", "y", "width", "height")}
            checks.append({"Scene": scene, "Control": name, "DeltaLogicalPixels": delta, "Passed": max(delta.values()) <= 2})
    report = {"Passed": bool(checks) and all(c["Passed"] for c in checks), "Checks": checks,
              "Reference": "Unchanged Wails frontend DOM, isolated synthetic service, 640x660 CSS viewport",
              "Scope": "Core rectangle tolerance <=2 logical pixels; not physical DPI, native frame or typography acceptance"}
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if not report["Passed"]:
        raise ValueError("Core rectangles differ from Wails reference")
    print(f"Passed {len(checks)} independent Wails/Slint rectangle checks.")


if __name__ == "__main__":
    main()
