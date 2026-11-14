"""Publish named native window recordings with their original clocks and limits.

This never marks RS-13 complete. Physical input, geometric tolerance and matching
the Wails animation are acceptance decisions made from the whole evidence set.
"""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("recording_directory", type=Path)
    parser.add_argument("session_json", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if args.output_directory.exists():
        raise ValueError("Use a fresh output directory; preserve earlier attempts")
    session = json.loads(args.session_json.read_text(encoding="utf-8-sig"))
    events = [json.loads(line) for line in (args.recording_directory / "events.jsonl").read_text(encoding="utf-8-sig").splitlines() if line]
    ready = next(e for e in events if e["event"] == "capture_ready")
    if ready["pid"] != session["ProcessId"] or ready["creation_filetime_100ns"] != session["StartedTicks"] - 504911232000000000:
        raise ValueError("Recorder attached to a different process creation identity")
    if ready.get("backend") != "WindowsGraphicsCapture":
        raise ValueError("This publisher expects the selected-window capture schema")
    frequency = ready["qpc_frequency"]
    clips = []
    outputs = []
    for start in (e for e in events if e["event"] == "clip_start"):
        name = start["clip"]
        if Path(name).name != name or not name.replace("-", "").isalnum():
            raise ValueError("Unexpected clip name")
        rows = [e for e in events if e["event"] == "frame" and e.get("clip") == name and e.get("saved")]
        images = []
        hashes = []
        for row in rows:
            path = args.recording_directory / name / f"frame-{row['index']}.bmp"
            with Image.open(path) as image:
                if image.size != (row["width"], row["height"]):
                    raise ValueError("Raw frame size differs from its trace")
                images.append(image.convert("RGB"))
            hashes.append({"File": f"{name}/{path.name}", "SHA256": sha(path), "Bytes": path.stat().st_size})
        changed_intervals = 0
        major_intervals = []
        for index, (left, right) in enumerate(zip(images, images[1:]), 1):
            if left.size != right.size:
                major_intervals.append(index)
                continue
            difference = ImageChops.difference(left, right)
            if difference.getbbox():
                changed_intervals += 1
                # Ignore a text caret when trimming idle lead/trail time. The
                # original timestamps, frames and hashes remain authoritative.
                channels = difference.split()
                mask = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
                if sum(mask.histogram()[8:]) > 200:
                    major_intervals.append(index)
        possible_limit = len(rows) >= 128 or (bool(images) and sum(h["Bytes"] for h in hashes) + images[-1].width * images[-1].height * 4 > 128 * 1024 * 1024)
        private = name.startswith("private-")
        item = {"Name": name, "SavedFrames": len(rows), "ChangedIntervals": changed_intervals,
                "MajorChangeIntervals": len(major_intervals), "PossibleStorageLimit": possible_limit,
                "NativeDPI": sorted({r["dpi"] for r in rows}), "OriginalFrames": hashes,
                "Published": bool(images) and not private,
                "Clock": "Original WGC surface time and observer QPC; not final desktop/photon time"}
        if len(rows) > 1:
            if any(a["observed_qpc"] >= b["observed_qpc"] for a, b in zip(rows, rows[1:])):
                raise ValueError("Observer clock is not monotonic")
            item["ObservedSpanMilliseconds"] = (rows[-1]["observed_qpc"] - rows[0]["observed_qpc"]) * 1000 / frequency
            item["MaximumObservedGapMilliseconds"] = max(b["observed_qpc"] - a["observed_qpc"] for a, b in zip(rows, rows[1:])) * 1000 / frequency
            if all('frame_time_100ns' in row for row in rows):
                if any(a['frame_time_100ns'] >= b['frame_time_100ns'] for a, b in zip(rows, rows[1:])):
                    raise ValueError('WGC source clock is not monotonic')
                live = [row for row in rows if not row.get('baseline_cached')]
                item['MaximumSourceGapMilliseconds'] = max((b['frame_time_100ns'] - a['frame_time_100ns']) / 10000 for a, b in zip(live, live[1:])) if len(live) > 1 else None
        if images and not private:
            first = max(0, major_intervals[0] - 1) if major_intervals else 0
            last = min(len(images) - 1, major_intervals[-1] + 1) if major_intervals else len(images) - 1
            selected = images[first:last + 1]
            canvas_size = (max(i.width for i in selected), max(i.height for i in selected))
            padded = []
            for image in selected:
                frame = Image.new("RGB", canvas_size, "#e8ecf2")
                frame.paste(image, (0, 0))
                padded.append(frame)
            source_clock = all('frame_time_100ns' in row for row in rows)
            durations = [max(1, round((b['frame_time_100ns'] - a['frame_time_100ns']) / 10000 if source_clock else (b["observed_qpc"] - a["observed_qpc"]) * 1000 / frequency))
                         for a, b in zip(rows[first:last + 1], rows[first + 1:last + 1])] + [200]
            outputs.append((name, padded, durations))
            item["PreviewOriginalFrameRange"] = [first, last]
            item["PreviewClock"] = ("WGC source intervals" if source_clock else "Actual observer intervals") + ", idle lead/trail trimmed; last frame held 200ms for review"
            item["PreviewScaled"] = False
        clips.append(item)
    args.output_directory.mkdir()
    for name, frames, durations in outputs:
        frames[0].save(args.output_directory / f"{name}.webp", save_all=True, append_images=frames[1:],
                       duration=durations, loop=0, lossless=True)
        # A short contact strip helps inspect transitions without assuming the
        # reviewer's viewer supports animated WebP.
        indices = sorted({0, len(frames) // 4, len(frames) // 2, 3 * len(frames) // 4, len(frames) - 1})
        width = min(320, frames[0].width)
        height = round(frames[0].height * width / frames[0].width)
        sheet = Image.new("RGB", (len(indices) * width, height + 24), "#e8ecf2")
        draw = ImageDraw.Draw(sheet)
        for position, index in enumerate(indices):
            draw.text((position * width + 6, 4), f"saved frame {index}", fill="black")
            sheet.paste(frames[index].resize((width, height)), (position * width, 24))
        sheet.save(args.output_directory / f"{name}-contact.png")
    report = {"Edition": session["Edition"], "ExecutableSHA256": session["SHA256"],
              "ProcessId": session["ProcessId"], "StartedTicks": session["StartedTicks"],
              "CaptureReady": ready, "CollectorSHA256": sha(Path(__file__)),
              "RawTraceSHA256": sha(args.recording_directory / "events.jsonl"),
              "Ended": any(e["event"] == "end" for e in events), "Clips": clips,
              "PhysicalInputProvenByThisFile": False, "RS13Complete": False,
              "Scope": "Native selected-window surface records. Stimulus identity and physical-input proof belong to the operator/session logs. Preview trimming/padding never changes the raw frames or original clocks."}
    (args.output_directory / "recordings.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Published {len(outputs)} native window clip previews; all limits and source identities retained.")
    if not outputs:
        raise ValueError("No saved native clip frames; this attempt is incomplete")


if __name__ == "__main__":
    main()
