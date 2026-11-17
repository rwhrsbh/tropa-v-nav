"""Writes docs/tools.md: every script in tools/ with what it is for and how it is run, read off
the script's own docstring (or a JavaScript file's first comment).

    python tools/make_tools_doc.py

Run it after adding or changing a tool; the page groups the tools by what they work on. A tool
whose docstring is missing shows up under "Undocumented": write one.
"""
import ast
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
TOOLS = os.path.join(ROOT, "tools")

GROUPS = [
    ("Foes: rig parts, corpses, poses", ["flow_rig_cmds", "rig_in", "rigparts", "rig_fill", "key_flood", "flow_pose_cmds", "pose_batch_in", "pose_in", "foe_rig", "slice_all_rigs", "slice_heads", "cutout", "trim_tail"]),
    ("Pictures: cards, relics, icons, story", ["flowart", "flow_next", "flow_page", "gen_art", "cardart", "cardsheet", "cards3", "cards4", "relicart", "relic_prompts", "intentart", "icon_api", "omenart", "placeart", "bookart", "inkcut", "labelsheet", "sheet", "collect", "wave_prompts", "old_letters_font"]),
    ("Voice", ["tts_api", "tts_story", "tts_omens", "voice_in", "hear", "find_repeats", "redo_doubled", "voice_join", "voice_split", "barkpage", "barkjs", "barkftl"]),
    ("Sound and music", ["sfx_api", "gen_sfx", "gen_music", "music_in"]),
    ("Text", ["check_strings"]),
    ("Release and analytics", ["pack_assets", "google_sheets_telemetry", "make_tools_doc"]),
]


def doc_of(path):
    text = open(path, encoding="utf-8", errors="replace").read()
    if path.endswith(".py"):
        try:
            return ast.get_docstring(ast.parse(text)) or ""
        except SyntaxError:
            return ""
    m = re.match(r"\s*/\*\*?(.*?)\*/", text, re.S)
    if m:
        return "\n".join(l.strip().lstrip("*").rstrip() for l in m.group(1).splitlines()).strip()
    lines = []
    for l in text.splitlines():
        if l.startswith("//"):
            lines.append(l[2:].strip())
        elif lines:
            break
    return "\n".join(lines)


def main():
    files = {os.path.splitext(f)[0]: f for f in sorted(os.listdir(TOOLS)) if f.endswith((".py", ".js")) and not f.startswith("_")}
    out = ["# Tools", "",
           "Every script in `tools/`, what it is for and how it is run, taken from its own docstring",
           "(`python tools/make_tools_doc.py` writes this page). The step-by-step recipes that string them",
           "together are in [art_pipeline.md](art_pipeline.md), [audio_pipeline.md](audio_pipeline.md),",
           "[i18n.md](i18n.md) and [release.md](release.md). Run Python tools from the repository root with",
           "`python -X utf8 tools/<name>.py`.", ""]
    seen = set()
    groups = GROUPS + [("Undocumented or other", [k for k in files if not any(k in g for _, g in GROUPS)])]
    for title, names in groups:
        names = [n for n in names if n in files and n not in seen]
        if not names:
            continue
        out += [f"## {title}", ""]
        for n in names:
            seen.add(n)
            d = doc_of(os.path.join(TOOLS, files[n]))
            out += [f"### `{files[n]}`", ""]
            if d:
                # usage lines (indented) as a code block, the rest as text
                prose, code = [], []
                for l in d.splitlines():
                    if l.startswith("    ") and l.strip():
                        code.append(l[4:])
                    else:
                        if code:
                            prose.append("```\n" + "\n".join(code) + "\n```")
                            code = []
                        prose.append(l)
                if code:
                    prose.append("```\n" + "\n".join(code) + "\n```")
                out += ["\n".join(prose).strip(), ""]
            else:
                out += ["_No docstring yet._", ""]
    open(os.path.join(ROOT, "docs", "tools.md"), "w", encoding="utf-8", newline="\n").write("\n".join(out).rstrip() + "\n")
    print(len(seen), "tools")


if __name__ == "__main__":
    main()
