"""Keeps the first saying of a clip the speech model said twice, or began to say again.

    python tools/voice_once.py <file.wav> [...]          # cut these at their longest inner pause
    python tools/voice_once.py --scan <folder> [<who>]   # list clips that look said twice
    python tools/voice_once.py --text <lang> <key> ...   # keep only the line's first saying, found
                                                         # by Whisper (also drops a prompt read aloud)

A doubled clip has a long pause in its middle (over 0.6 s) with speech on both sides; the cut is
made just after the first saying, with a short fade. The scan only lists: listen before cutting,
since a slow line ("Тише… тише…") pauses on purpose.
"""
import glob
import os
import sys
import wave

import numpy as np

WIN = 0.03


def load(path):
    w = wave.open(path)
    params = w.getparams()
    a = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(float)
    w.close()
    return params, a


def gap(a, sr):
    """The longest pause inside the clip, as (start, end) in seconds, with speech on both sides."""
    win = int(sr * WIN)
    e = np.array([np.abs(a[i:i + win]).mean() for i in range(0, max(1, len(a) - win), win)])
    quiet = e < e.max() * 0.04
    runs, start = [], None
    for i, q in enumerate(quiet):
        if q and start is None:
            start = i
        if not q and start is not None:
            runs.append((start, i))
            start = None
    inner = [r for r in runs if r[0] > 3 and r[1] < len(e) - 3]
    if not inner:
        return None
    s, t = max(inner, key=lambda r: r[1] - r[0])
    return s * WIN, t * WIN


def cut(path):
    params, a = load(path)
    sr = params.framerate
    g = gap(a, sr)
    if not g:
        print("no pause", path)
        return
    end = int(sr * (g[0] + 0.15))
    a = a[:end]
    fade = int(sr * 0.05)
    a[-fade:] *= np.linspace(1, 0, fade)
    w = wave.open(path, "wb")
    w.setparams(params)
    w.writeframes(a.astype(np.int16).tobytes())
    w.close()
    print("cut", path, "to %.2fs" % (end / sr))


def by_text(lang, key):
    """Crops assets/voice/<lang>/<key>.wav to the first stretch where Whisper hears the line's
    words: a line said twice keeps its first saying, a line read after the prompt keeps the line."""
    import re
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import hear
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    path = os.path.join(root, "assets", "voice", lang, key + ".wav")
    text = dict(re.findall(r"^(bark-[a-z0-9-]+) = (.*)$", open(os.path.join(root, "assets", "i18n", lang, "voice.ftl"), encoding="utf-8").read(), re.M))[key]
    hear.hear(path, "en" if lang == "en" else "ru")
    params, a = load(path)
    sr = params.framerate
    pcm = (a / 32768.0).astype(np.float32)
    if sr != 16000:
        at = np.arange(0, len(pcm), sr / 16000.0)
        pcm = np.interp(at, np.arange(len(pcm)), pcm).astype(np.float32)
    segs, _ = hear.MODEL.transcribe(pcm, language="en" if lang == "en" else "ru", beam_size=5, word_timestamps=True)
    heard = [(w.start, w.end, hear.words(w.word)) for s in segs for w in s.words]
    want = hear.words(text)
    flat = [(st, en, w) for st, en, ws in heard for w in ws]
    # the first run of heard words that covers the line's first and last word, in order
    best = None
    for i, (st, _, w) in enumerate(flat):
        if w[:4] != want[0][:4]:
            continue
        for j in range(i, len(flat)):
            if flat[j][2][:4] == want[-1][:4] and j - i + 1 >= len(want) * 0.6:
                best = (st, flat[j][1])
                break
        if best:
            break
    if not best:
        print("line not found", key, [w for _, _, w in flat])
        return
    s0, s1 = max(0, int((best[0] - 0.12) * sr)), min(len(a), int((best[1] + 0.25) * sr))
    a = a[s0:s1]
    fade = int(sr * 0.04)
    a[:fade] *= np.linspace(0, 1, fade)
    a[-fade:] *= np.linspace(1, 0, fade)
    w = wave.open(path, "wb")
    w.setparams(params)
    w.writeframes(a.astype(np.int16).tobytes())
    w.close()
    print("kept %.2f-%.2f" % best, key)


def main():
    if sys.argv[1] == "--text":
        for key in sys.argv[3:]:
            by_text(sys.argv[2], key)
        return
    if sys.argv[1] == "--scan":
        who = sys.argv[3] if len(sys.argv) > 3 else ""
        for p in sorted(glob.glob(os.path.join(sys.argv[2], f"bark-{who}*.wav"))):
            params, a = load(p)
            g = gap(a, params.framerate)
            if g and g[1] - g[0] > 0.6:
                print("%.2f-%.2f of %.2fs" % (g[0], g[1], len(a) / params.framerate), p)
        return
    for p in sys.argv[1:]:
        cut(p)


if __name__ == "__main__":
    main()
