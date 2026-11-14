"""Verify and publish timestamped Slint component recordings, not desktop video."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def energy(left, right):
    return sum(ImageStat.Stat(ImageChops.difference(left, right)).sum)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if args.output_directory.exists():
        raise ValueError("Preserve previous evidence; use a fresh output directory")
    inputs = {}
    comparisons = []
    source_ui = None
    for renderer in ("software", "femtovg"):
        directory = args.capture_directory / renderer
        manifest = json.loads((directory / "capture.json").read_text(encoding="utf-8-sig"))
        if sha(directory / "frames/recording.json") != manifest["RecordingSHA256"]:
            raise ValueError("Recording hash differs")
        recording = json.loads((directory / "frames/recording.json").read_text(encoding="utf-8"))
        if not recording["Complete"] or not recording["SyntheticInput"] or recording["PresentedFrames"]:
            raise ValueError("Unexpected recording provenance")
        ui = [row for row in manifest["Sources"] if row["Path"].startswith("ui/")]
        if source_ui is not None and source_ui != ui:
            raise ValueError("Renderer UI inputs differ")
        source_ui = ui
        expected = {row["Name"]: row["SHA256"] for row in manifest["Images"]}
        if set(expected) != {frame["name"] for frame in recording["Frames"]}:
            raise ValueError("Recording frame inventory differs")
        for row in recording["Frames"]:
            name = row["name"]
            if Path(name).name != name or sha(directory / "frames" / name) != expected[name]:
                raise ValueError("Unsafe name or changed frame bytes")
        for clip in recording["Clips"]:
            frames = [row for row in recording["Frames"] if row["clip"] == clip]
            if len(frames) < 20 or any(a["elapsed_us"] >= b["elapsed_us"] for a, b in zip(frames, frames[1:])):
                raise ValueError("Insufficient frames or nonmonotonic snapshot clock")
            images = []
            for row in frames:
                with Image.open(directory / "frames" / row["name"]) as image:
                    images.append(image.convert("RGB"))
            if len({image.size for image in images}) != 1:
                raise ValueError("Geometry changed during a fixed-size clip")
            changes = sum(energy(a, b) > 0 for a, b in zip(images, images[1:]))
            static = clip.startswith("reduced-") or clip == "hidden-scanning"
            if static and (changes or any(row["active_animations"] for row in frames[-5:])):
                raise ValueError(f"Decorations remain active in {renderer}/{clip}")
            if not static and changes < 2:
                raise ValueError(f"No captured transition in {renderer}/{clip}")
            item = {
                "Renderer": renderer, "Clip": clip, "Frames": len(frames),
                "ChangedIntervals": changes, "StaticExpected": static,
                "FinalActiveAnimations": frames[-1]["active_animations"],
                "ElapsedMilliseconds": (frames[-1]["elapsed_us"] - frames[0]["elapsed_us"]) / 1000,
                "MaximumSnapshotGapMilliseconds": max(b["elapsed_us"] - a["elapsed_us"] for a, b in zip(frames, frames[1:])) / 1000,
            }
            if clip == "home-entry":
                # Compare normalized opacity progress of the same column in two
                # rows. Different glyphs/selection colors are normalized against
                # each tile's own initial and stable images, rather than each other.
                index = min(range(len(frames)), key=lambda i: abs(frames[i]["elapsed_us"] - 100_000))
                remaining = []
                for box in ((124, 198, 214, 284), (124, 294, 214, 380)):
                    initial, current, final = (image.crop(box) for image in (images[0], images[index], images[-1]))
                    total = energy(initial, final)
                    if total == 0:
                        raise ValueError("Home entry clip does not cover both tile rows")
                    remaining.append(energy(current, final) / total)
                item["SameColumnProgressDifference"] = abs(remaining[0] - remaining[1])
                item["ComparedAtMilliseconds"] = frames[index]["elapsed_us"] / 1000
                if item["SameColumnProgressDifference"] > 0.15:
                    raise ValueError("Second row has a different entry delay from the same column")
            comparisons.append(item)
            durations = [max(1, round((b["elapsed_us"] - a["elapsed_us"]) / 1000)) for a, b in zip(frames, frames[1:])] + [40]
            inputs[renderer, clip] = (images, durations)
    args.output_directory.mkdir()
    for renderer in ("software", "femtovg"):
        destination = args.output_directory / renderer
        destination.mkdir()
        for filename in ("capture.json", "frames/recording.json"):
            shutil.copyfile(args.capture_directory / renderer / filename, destination / Path(filename).name)
        for (edition, clip), (images, durations) in inputs.items():
            if edition == renderer:
                images[0].save(destination / f"{clip}.webp", save_all=True, append_images=images[1:], duration=durations, loop=0, lossless=True)
    report = {
        "Passed": True, "Clips": len(comparisons), "SourceMatched": True,
        "SyntheticComponentEvents": True, "PhysicalInput": False, "PresentedFrames": False,
        "RS13Complete": False, "SummarizerSHA256": sha(Path(__file__)),
        "Scope": "Lossless WebP previews use actual snapshot intervals. Snapshot/encoding overhead prevents performance or native presentation conclusions.",
        "Checks": comparisons,
    }
    (args.output_directory / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Verified and published {len(comparisons)} component recordings; native/IME/DPI acceptance remains separate.")


if __name__ == "__main__":
    main()
