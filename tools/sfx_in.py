"""Takes a sound effect downloaded from the ElevenLabs website into the game, cut the same way
tools/sfx_api.py cuts the ones it makes through the API.

    python tools/sfx_in.py "<downloaded .mp3>" <name>     # e.g. ... hollow_pull

Writes assets/sfx/<name>-flow.wav (mono 44.1 kHz, silence trimmed from both ends with soft edges)
and moves the download into _inbox/sfx/raw/.
"""
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sfx_api import ROOT, cut_silence, save, to_samples  # noqa: E402


def main():
    src, name = sys.argv[1], sys.argv[2]
    a = cut_silence(to_samples(src))
    dst = os.path.join(ROOT, "assets", "sfx", f"{name}-flow.wav")
    save(dst, a)
    raw = os.path.join(ROOT, "_inbox", "sfx", "raw")
    os.makedirs(raw, exist_ok=True)
    shutil.move(src, os.path.join(raw, f"{name}_{os.path.basename(src)}"))
    print(name, "%.2fs" % (len(a) / 44100))


if __name__ == "__main__":
    main()
