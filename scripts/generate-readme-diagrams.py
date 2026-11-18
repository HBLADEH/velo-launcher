"""Generate the two README SVGs with Python's standard library."""

import argparse
from html import escape
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LABELS = {
    "zh": {
        "title": "从呼出到启动，都在本机",
        "subtitle": "输入只查内存；扫描、系统交互与保存交给后台。",
        "process": "Velo · 单进程 Rust + Slint",
        "thread": "界面线程 · 输入与模型更新",
        "ui": ("Slint 界面", "ui/", "Alt+Space · 搜索 · 选择结果"),
        "adapter": ("Rust 界面适配", "src/business.rs · src/ui.rs", "回调、显示行与会话状态"),
        "search": ("内存搜索快照", "src/core/search.rs", "名称 · 拼音 · 历史排序"),
        "native": ("Windows 平台层", "src/platform/windows/", "窗口 · 热键 · Shell · COM"),
        "workers": ("有界后台任务", "src/core/service.rs", "扫描 · 图标 · 启动 · 写入"),
        "storage": ("原子存储", "src/core/storage.rs", "临时文件 → flush → 替换"),
        "callbacks": "回调 / 模型",
        "query": "查询 / 结果",
        "events": "命令 / 事件",
        "window": "热键 / 窗口",
        "index": "索引快照更新",
        "commit": "串行提交",
        "ipc": "可选 IPC",
        "files": "本地文件",
        "everything": ("Everything 默认实例", "可选外部进程 · 默认关闭"),
        "data": ("%LOCALAPPDATA%\\Velo", "配置 · 索引 · 历史 · 应用库"),
        "boundary": "后台结果回到事件循环；后台线程不直接修改 Slint 组件。",
        "footer": "应用搜索不扫描磁盘 · 图标缓存有预算 · 过期结果按 generation 丢弃",
        "desc": "单进程内，Slint 通过 Rust 界面适配查询内存快照；有界后台任务负责索引、Windows 启动与原子保存。外部 Everything 为默认关闭的可选连接。",
    },
    "en": {
        "title": "From hotkey to launch, on your PC",
        "subtitle": "Search in memory. Scan, open and save in the background.",
        "process": "Velo · Single-process Rust + Slint",
        "thread": "UI thread · Input and model updates",
        "ui": ("Slint interface", "ui/", "Alt+Space · Type · Select"),
        "adapter": ("Rust UI adapters", "src/business.rs · src/ui.rs", "Callbacks, rows and session state"),
        "search": ("In-memory search", "src/core/search.rs", "Names · Pinyin · History ranking"),
        "native": ("Windows platform", "src/platform/windows/", "Window · Hotkey · Shell · COM"),
        "workers": ("Bounded workers", "src/core/service.rs", "Scan · Icons · Launch · Write"),
        "storage": ("Atomic storage", "src/core/storage.rs", "Temp file → flush → replace"),
        "callbacks": "Callbacks / rows",
        "query": "Query / results",
        "events": "Commands / events",
        "window": "Hotkey / window",
        "index": "Index snapshot",
        "commit": "Serial writes",
        "ipc": "Optional IPC",
        "files": "Local files",
        "everything": ("Default Everything instance", "Optional process · Off by default"),
        "data": ("%LOCALAPPDATA%\\Velo", "Config · Index · History · Library"),
        "boundary": "Worker results return to the event loop; workers never update Slint components directly.",
        "footer": "No disk scan while typing · Bounded icon cache · Generation checks reject stale results",
        "desc": "Inside a single process, Slint calls Rust adapters to query an in-memory snapshot. Bounded workers handle indexing, Windows launches and atomic writes. External Everything integration is optional and disabled by default.",
    },
}


def label(x, y, value, css="label", anchor="start"):
    return f'<text x="{x}" y="{y}" class="{css}" text-anchor="{anchor}">{escape(value)}</text>'


def card(x, y, lines, accent, external=False):
    height = 84 if external else 124
    parts = [
        f'<rect x="{x}" y="{y}" width="288" height="{height}" rx="14" class="card"/>',
        f'<rect x="{x}" y="{y + 20}" width="4" height="{height - 40}" rx="2" fill="{accent}"/>',
    ]
    if external:
        parts += [label(x + 20, y + 33, lines[0], "external-title"), label(x + 20, y + 60, lines[1], "detail")]
    else:
        parts += [
            label(x + 20, y + 35, lines[0], "card-title"),
            label(x + 20, y + 66, lines[1], "code"),
            label(x + 20, y + 98, lines[2], "detail"),
        ]
    return "\n".join(parts)


def edge(path, x, y, caption, both=False, optional=False, anchor="middle"):
    start = ' marker-start="url(#arrow)"' if both else ""
    dashed = ' stroke-dasharray="6 5"' if optional else ""
    return (
        f'<path d="{path}" class="edge" marker-end="url(#arrow)"{start}{dashed}/>'
        + label(x, y, caption, "edge-label", anchor)
    )


def render(language):
    words = LABELS[language]
    fonts = '"Microsoft YaHei","Segoe UI",sans-serif' if language == "zh" else '"Segoe UI","Microsoft YaHei",sans-serif'
    parts = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="860" viewBox="0 0 1200 860" role="img" aria-labelledby="title desc">',
        f'<title id="title">{escape(words["title"])}</title>',
        f'<desc id="desc">{escape(words["desc"])}</desc>',
        '<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#64748b"/></marker></defs>',
        '<style>',
        f'text{{font-family:{fonts};fill:#162b45}}',
        '.title{font-size:30px;font-weight:700}.subtitle{font-size:18px;fill:#52657b}',
        '.process{font-size:20px;font-weight:600}.label{font-size:16px;fill:#52657b}',
        '.card{fill:white;stroke:#ced9e6;stroke-width:1.5}.card-title{font-size:22px;font-weight:600}',
        '.code{font-family:Consolas,"Cascadia Code",monospace;font-size:15px;fill:#476380}',
        '.detail{font-size:16px;fill:#52657b}.external-title{font-size:18px;font-weight:600}',
        '.edge{fill:none;stroke:#64748b;stroke-width:2}.edge-label{font-size:13px;fill:#52657b}',
        '.boundary{font-size:15px;fill:#52657b}.footer{font-size:16px;fill:#52657b}',
        '</style>',
        '<rect width="1200" height="860" rx="24" fill="#f7f9fc"/>',
        label(52, 58, words["title"], "title"),
        label(52, 94, words["subtitle"], "subtitle"),
        '<rect x="32" y="124" width="1136" height="558" rx="22" fill="#edf3fb" stroke="#c7d7eb" stroke-width="1.5"/>',
        label(64, 164, words["process"], "process"),
        label(64, 202, words["thread"]),
        card(64, 228, words["ui"], "#2979d7"),
        card(456, 228, words["adapter"], "#2979d7"),
        card(848, 228, words["search"], "#2979d7"),
        card(64, 494, words["native"], "#16847a"),
        card(456, 494, words["workers"], "#16847a"),
        card(848, 494, words["storage"], "#16847a"),
        edge("M 352 290 H 456", 404, 277, words["callbacks"], both=True),
        edge("M 744 290 H 848", 796, 277, words["query"], both=True),
        edge("M 600 352 V 494", 614, 423, words["events"], both=True, anchor="start"),
        edge("M 208 494 V 352", 222, 423, words["window"], anchor="start"),
        edge("M 456 548 H 352", 404, 535, "Shell / COM"),
        edge("M 744 586 H 848", 796, 573, words["commit"]),
        edge("M 744 520 H 790 V 391 H 992 V 352", 997, 414, words["index"]),
        label(600, 658, words["boundary"], "boundary", "middle"),
        card(64, 730, words["everything"], "#8b6bc5", external=True),
        card(848, 730, words["data"], "#8b6bc5", external=True),
        edge("M 208 618 V 730", 222, 707, words["ipc"], both=True, optional=True, anchor="start"),
        edge("M 992 618 V 730", 1006, 707, words["files"], anchor="start"),
        label(600, 849, words["footer"], "footer", "middle"),
        '</svg>',
    ]
    return "\n".join(parts) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Check generated SVGs without writing them")
    args = parser.parse_args()
    for language, filename in (("zh", "velo-how-it-works.svg"), ("en", "velo-how-it-works.en.svg")):
        path = ROOT / "docs" / "diagrams" / filename
        svg = render(language)
        if args.check:
            if not path.is_file() or path.read_text(encoding="utf-8") != svg:
                raise SystemExit(f"Regenerate {path.relative_to(ROOT)}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(svg, encoding="utf-8", newline="\n")
        print(f"{'Checked' if args.check else 'Generated'} {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
