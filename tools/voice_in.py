"""Takes voice clips downloaded from Google AI Studio into the game and checks them by ear.

    python tools/voice_in.py <ru-old|ru|en> <text key> [...]   # -> assets/voice/<lang>/<key>.wav
    python tools/voice_in.py --named                          # every tropa__<lang>__<key>.wav waiting

A clip the page helper saved under its own name (`tropa__<lang>__<key>.wav`) says which line it
is, so clips from several tabs at once can be taken in whatever order they arrived.

AI Studio saves every clip as `Generated Audio <date>.wav`; the waiting ones are taken in the
order they arrived, one per key. Each clip is then heard by a local Whisper (tools/hear.py) and
scored against the line it should say, taken from assets/i18n/<lang>/*.ftl: a clip that was
saved before the model finished, or that dropped words, shows as a low score and is left in
Downloads under `REJECTED <key>.wav` instead of going into the game. A good clip is levelled on the
way in (see `level`), because the model's voice sinks towards the end of a page; loudness by fifths
of the clip is printed before and after.
"""
import glob
import os
import re
import shutil
import sys
import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hear import hear, score  # noqa: E402
from trim_tail import trim  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.environ.get("TROPA_DOWNLOADS") or os.path.join(os.path.expanduser("~"), "Downloads")
# the old speech is Russian to Whisper, which then mishears its archaic endings: ask less of it
ENOUGH = {"ru-old": 0.35, "ru": 0.8, "en": 0.8}
# characters a second the narrator cannot honestly exceed; a faster clip has lost its end
FASTEST = {"ru-old": 10, "ru": 13, "en": 17}


def lines(lang):
    out = {}
    # the old speech overrides Russian, as it does in the game
    for folder in (["ru"] if lang == "ru-old" else []) + [lang]:
        for f in glob.glob(os.path.join(ROOT, "assets", "i18n", folder, "*.ftl")):
            for line in open(f, encoding="utf-8"):
                m = re.match(r"([a-z0-9_-]+) = (.*)", line)
                if m:
                    out[m.group(1)] = m.group(2).strip()
    return out


# loudness every clip is brought to (RMS of its spoken parts, 16-bit scale)
LOUD = 3600.0


def level(frames, rate):
    """Evens a clip out: the same loudness from its first word to its last, and from clip to clip.

    The speech model lets its voice sink towards the end of a page. The clip is measured second by
    second (only where someone is speaking), each stretch is brought to the common loudness, and the
    gain glides between stretches so nothing pumps or clicks.
    """
    frames = frames.astype(np.float32)
    mono = np.abs(frames).mean(axis=1) if frames.ndim > 1 else np.abs(frames)
    hop = rate // 50
    n = max(1, len(mono) // hop)
    env = np.array([np.sqrt((mono[i * hop:(i + 1) * hop] ** 2).mean()) for i in range(n)])
    speaking = env > max(env.max() * 0.08, 60.0)
    if not speaking.any():
        return frames
    # loudness around every moment: spoken frames within a second and a half either side
    half = 75
    gains = np.ones(n, dtype=np.float32)
    for i in range(n):
        a, b = max(0, i - half), min(n, i + half + 1)
        near = env[a:b][speaking[a:b]]
        if len(near) >= 10:
            gains[i] = LOUD / max(float(np.sqrt((near ** 2).mean())), 1.0)
        else:
            gains[i] = np.nan
    # silences take the gain of the speech beside them
    known = np.flatnonzero(~np.isnan(gains))
    gains = np.interp(np.arange(n), known, gains[known]).clip(0.4, 6.0)
    # glide: half a second of smoothing
    kernel = np.hanning(51)
    gains = np.convolve(np.pad(gains, 25, mode="edge"), kernel / kernel.sum(), mode="valid")
    per_sample = np.interp(np.arange(len(mono)), (np.arange(n) + 0.5) * hop, gains)
    out = frames * (per_sample[:, None] if frames.ndim > 1 else per_sample)
    peak = float(np.abs(out).max())
    if peak > 31000.0:
        out *= 31000.0 / peak
    return out


def save_levelled(src, dst):
    with wave.open(src) as w:
        rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).reshape(-1, channels)
    with wave.open(dst, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(width)
        w.setframerate(rate)
        w.writeframes(level(raw, rate).astype(np.int16).tobytes())


def loudness(path):
    with wave.open(path) as w:
        a = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(float)
    n = max(1, len(a) // 5)
    return [int(np.sqrt((a[i * n:(i + 1) * n] ** 2).mean())) for i in range(5)]


def take(f, lang, key, text, reject_name):
    """Checks one clip against its line; a good one goes into the game, a bad one is set aside.

    With TROPA_NO_HEAR=1 Whisper is not run (it takes the graphics card): only the length is
    checked, and the clip is noted in _inbox/voice/unheard.txt to be heard later.
    """
    # some speech models end with a burst of hiss: cut it before anything else
    trim(f)
    if os.environ.get("TROPA_NO_HEAR"):
        with wave.open(f) as w:
            seconds = w.getnframes() / w.getframerate()
        heard = "(not heard)"
        os.makedirs(os.path.join(ROOT, "_inbox", "voice"), exist_ok=True)
        with open(os.path.join(ROOT, "_inbox", "voice", "unheard.txt"), "a", encoding="utf-8") as u:
            u.write(f"{lang} {key}\n")
        # unheard, only the length can be judged: take the words as said
        ok, s = passes(lang, key, text[key], text[key], seconds)
    else:
        heard, seconds = hear(f, lang.split("-")[0])
        ok, s = passes(lang, key, text[key], heard, seconds)
    if ok:
        out_dir = os.path.join(ROOT, "assets", "voice", lang)
        os.makedirs(out_dir, exist_ok=True)
        out = os.path.join(out_dir, f"{key}.wav")
        save_levelled(f, out)
        print(f"ok  {lang} {key}: {seconds:.1f}s heard {s:.0%}    {heard}")
        os.makedirs(os.path.join(ROOT, "_inbox", "voice"), exist_ok=True)
        shutil.move(f, os.path.join(ROOT, "_inbox", "voice", f"{lang}_{key}_take.wav"))
    else:
        print(f"BAD {lang} {key}: {seconds:.1f}s heard {s:.0%}    heard: {heard}")
        shutil.move(f, os.path.join(DOWNLOADS, reject_name))
    return ok


def passes(lang, key, text, heard, seconds):
    """Would `take` accept a clip of this length in which Whisper heard this?"""
    s = score(text, heard)
    # a short battle cry gives Whisper little to go on: ask less of it
    ok = s >= ENOUGH.get(lang, 0.85) * (0.6 if key.startswith("bark-") else 1.0)
    # one or two words: Whisper cannot judge them, a clip of the right length will do
    if len(text.split()) <= 2:
        ok = seconds >= 0.8
    # a clip cut short is also much shorter than its text takes to say
    return ok and seconds * FASTEST.get(lang, 17) >= len(text), s


def rehear():
    """Listens at last to the clips taken in unheard (TROPA_NO_HEAR) and lists the doubtful ones.

    Nothing is moved: the doubtful clips stay in the game until someone listens to them and records
    them again. Heard clips leave unheard.txt; the doubtful go to _inbox/voice/doubtful.txt.
    """
    inbox = os.path.join(ROOT, "_inbox", "voice")
    todo_path = os.path.join(inbox, "unheard.txt")
    with open(todo_path, encoding="utf-8") as u:
        seen = []
        for line in u:
            parts = tuple(line.split())
            if len(parts) == 2 and parts not in seen:
                seen.append(parts)
    texts, doubtful = {}, []
    for lang, key in seen:
        clip = os.path.join(ROOT, "assets", "voice", lang, f"{key}.wav")
        if not os.path.exists(clip):
            continue
        texts.setdefault(lang, lines(lang))
        text = texts[lang].get(key)
        if text is None:
            continue
        heard, seconds = hear(clip, lang.split("-")[0])
        ok, s = passes(lang, key, text, heard, seconds)
        print(f"{'ok ' if ok else 'BAD'} {lang} {key}: {seconds:.1f}s heard {s:.0%}    {heard}")
        if not ok:
            doubtful.append(f"{lang} {key}\t{text}\theard: {heard}\n")
    with open(os.path.join(inbox, "doubtful.txt"), "a", encoding="utf-8") as d:
        d.writelines(doubtful)
    open(todo_path, "w", encoding="utf-8").close()
    print(f"{len(seen)} heard, {len(doubtful)} doubtful (listed in _inbox/voice/doubtful.txt)")


def main():
    if sys.argv[1:2] == ["--unheard"]:
        rehear()
        return
    if sys.argv[1:2] == ["--named"]:
        texts = {}
        files = sorted(glob.glob(os.path.join(DOWNLOADS, "tropa__*__*.wav")), key=os.path.getmtime)
        good = 0
        for f in files:
            m = re.match(r"tropa__([a-z-]+)__([a-z0-9-]+)(?: \(\d+\))?\.wav$", os.path.basename(f))
            if not m:
                continue
            lang, key = m.groups()
            # a line already in the game is never replaced by a stray take
            if os.path.exists(os.path.join(ROOT, "assets", "voice", lang, f"{key}.wav")):
                print(f"has {lang} {key}: kept the old take")
                shutil.move(f, os.path.join(DOWNLOADS, f"SPARE {lang} {key}.wav"))
                continue
            texts.setdefault(lang, lines(lang))
            good += take(f, lang, key, texts[lang], f"REJECTED {lang} {key}.wav")
        print(f"{good} of {len(files)} taken in")
        return
    lang, keys = sys.argv[1], sys.argv[2:]
    files = sorted(glob.glob(os.path.join(DOWNLOADS, "Generated Audio*.wav")), key=os.path.getmtime)
    if len(files) < len(keys):
        raise SystemExit(f"{len(files)} clips waiting but {len(keys)} keys given; nothing moved")
    # the oldest ones are these keys; any newer clips wait for the next call
    files = files[:len(keys)]
    text = lines(lang)
    for f, key in zip(files, keys):
        take(f, lang, key, text, f"REJECTED {key}.wav")


if __name__ == "__main__":
    main()
