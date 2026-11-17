"""Unpacks the voice packs the AI Studio page helper saves (`tropa-voice-pack-*.bin`) into the
separate `tropa__<lang>__<key>.wav` clips voice_in.py takes in.

    python tools/voicepack.py

The browser lets a page save only one file without asking, so the page keeps every clip it
records and saves them together: a 4-byte big-endian header length, a JSON list of
`[name, size]`, then the clips one after another.
"""
import glob
import json
import os
import struct

DOWNLOADS = os.environ.get("TROPA_DOWNLOADS") or os.path.join(os.path.expanduser("~"), "Downloads")


def main():
    n = 0
    for f in sorted(glob.glob(os.path.join(DOWNLOADS, "tropa-voice-pack-*.bin")), key=os.path.getmtime):
        data = open(f, "rb").read()
        size = struct.unpack(">I", data[:4])[0]
        head = json.loads(data[4:4 + size].decode("utf-8"))
        at = 4 + size
        for name, length in head:
            open(os.path.join(DOWNLOADS, name), "wb").write(data[at:at + length])
            at += length
            n += 1
        os.remove(f)
    print(f"{n} clips unpacked")


if __name__ == "__main__":
    main()
