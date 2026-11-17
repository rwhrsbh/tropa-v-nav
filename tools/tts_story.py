"""Records story pages and book secrets through the Gemini API.

    python -X utf8 tools/tts_story.py <ru-old|ru|en> [page_ids ...]
"""
import os
import re
import sys
import time
import urllib.error
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tts_api import KEYS, MARKDOWN, VOICE, speak
from voice_in import DOWNLOADS, ROOT, lines, take

MODELS = ["gemini-3.1-flash-tts-preview", "gemini-3.8-flash-tts", "gemini-2.5-pro-preview-tts", "gemini-2.5-flash-preview-tts", "gemini-3.8-flash-lite-tts"]
SPEAKER = "Algenib"


def scene(lang):
    out = []
    started = False
    for line in open(os.path.join(VOICE, f"story.{lang}.txt"), encoding="utf-8"):
        if line.startswith("#   "):
            started = True
            out.append(line[4:].strip())
        elif started:
            break
    return " ".join(out)


def story_pages(lang):
    """Returns dict of {page_key: text_to_read}."""
    path = os.path.join(VOICE, f"story.{lang}.txt")
    out = {}
    if os.path.exists(path):
        for line in open(path, encoding="utf-8"):
            m = re.match(r"^(story-[a-z0-9_-]+) = (.*)$", line.strip())
            if m:
                out[m.group(1)] = m.group(2).strip()
    # Also check ftl files
    ftl_text = lines("ru" if lang == "ru-old" else lang)
    for k, v in ftl_text.items():
        if k.startswith("story-") and k not in out:
            out[k] = v
    return out


def main():
    args = sys.argv[1:]
    if not args:
        print("Usage: python tools/tts_story.py <ru-old|ru|en> [page_ids ...]")
        return
    lang = args[0]
    only = set(args[1:]) if len(args) > 1 else None

    keys = [k.strip() for k in open(KEYS, encoding="utf-8") if k.strip() and not k.startswith("#")]
    pairs = [(m, k) for m in MODELS for k in keys]
    spent, cool = set(), {}
    style = scene(lang)

    all_pages = story_pages(lang)
    target_dir = os.path.join(ROOT, "assets", "voice", lang)
    os.makedirs(target_dir, exist_ok=True)

    todo = []
    for k, text in all_pages.items():
        if only and k not in only and k.replace("story-", "") not in only:
            continue
        dest_file = os.path.join(target_dir, f"{k}.wav")
        if not os.path.exists(dest_file):
            todo.append((k, text))

    print(f"{lang}: {len(todo)} story pages to record")
    for name, read in todo:
        prompt = f"{style}\n\nRead this aloud, once, exactly as written:\n{read}"
        pcm = None
        for _ in range(40):
            now = time.time()
            live = [p for p in pairs if p not in spent and cool.get(p, 0) <= now]
            if not live:
                if all(p in spent for p in pairs):
                    print("All models out of quota for today")
                    return
                time.sleep(max(1.0, min(cool[p] for p in pairs if p not in spent) - now))
                continue
            model, key = live[0]
            try:
                laid_out = "## Style:\n" + style + "\n\n## Transcript:\n" + read
                pcm, rate = speak(key, model, SPEAKER, laid_out if model in MARKDOWN else prompt)
                break
            except urllib.error.HTTPError as e:
                raw = e.read().decode(errors="replace")
                if e.code == 429:
                    if "PerDay" in raw:
                        spent.add((model, key))
                    else:
                        m = re.search(r'"retryDelay":\s*"(\d+)', raw)
                        cool[(model, key)] = time.time() + (int(m.group(1)) if m else 60) + 1
                    continue
                if e.code in (403, 404):
                    spent.add((model, key))
                    continue
                print(f"{model}: {e.code} {raw[:200]!r}")
                time.sleep(5)
            except Exception as e:
                print(f"{model}: error ({e!r})")
                time.sleep(3)

        if pcm is None:
            print(f"Failed to record {name}")
            continue

        f = os.path.join(DOWNLOADS, f"api {lang} {name}.wav")
        with wave.open(f, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(rate)
            w.writeframes(pcm)
        print(model, end=" ")
        plain = re.sub(r"[А-ЯA-Z]", lambda m: m.group(0).lower(), read) if lang == "ru-old" else read
        take(f, lang, name, {name: plain}, f"REJECTED {lang} {name}.wav")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
