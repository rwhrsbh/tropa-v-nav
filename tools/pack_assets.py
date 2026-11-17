"""Packs the game's assets for shipping, smaller, into target/pack/<profile>/assets.

    python -X utf8 tools/pack_assets.py desktop   # by eye and ear the same: WebP q92, Ogg Vorbis
    python -X utf8 tools/pack_assets.py mobile    # by eye and ear the same: WebP q90, Ogg Vorbis
    python -X utf8 tools/pack_assets.py mobile --only cards   # one folder, to look at

What it does:
- leaves out every file the game never loads: a `name.png` shadowed by `name-gemini.png`, a
  `name.wav` shadowed by `name-flow.ogg`/`-flow.wav`, and the working folders below;
- turns pictures into WebP and sounds into FLAC or Ogg, one kind per file, never two side by side
  (the game looks for either kind: `Art::file`);
- writes `index.txt`, every packed file, which the game reads instead of the folder (on Android
  the folder cannot be read at all).

Pictures are never kept larger than the game shows them (`MAX_SIDE`): a card's art is drawn a
few hundred pixels wide at most, and was painted three times that.

A file already packed from the same source bytes with the same settings is kept (the packed
folder's `manifest.json` remembers what each was packed from), so a second run is quick, and a
packed folder restored from an older cache only redoes what changed since.
"""

import hashlib
import io
import json
import os
import sys
import shutil
from concurrent.futures import ProcessPoolExecutor

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
SRC = os.path.join(ROOT, "assets")
# working files that are never loaded by the game
SKIP_DIRS = {"_old", "_work", "_src", "originals", "raw"}
SKIP_FILES = {"Thumbs.db", ".DS_Store", "desktop.ini"}

PROFILES = {
    # (image save options, sound format, sound options). Lossy WebP at these qualities and Ogg
    # Vorbis cannot be told from the sources on a screen and in headphones, at a tenth the size
    # of WebP lossless and FLAC
    "desktop": (dict(quality=92, method=4, alpha_quality=100), "ogg", dict(subtype="VORBIS", compression_level=0.15)),
    "mobile": (dict(quality=90, method=4, alpha_quality=95), "ogg", dict(subtype="VORBIS", compression_level=0.3)),
}
# The longest side a picture in this folder is ever drawn at, on a 4K screen (card art is shown at
# most about 600 px wide there, in the book of cards)
MAX_SIDE = {"cards/": 720}
# bumped whenever the way files are packed changes: everything is packed again
PACK_VERSION = 2


def shadowed(dirpath, name, names):
    stem, ext = os.path.splitext(name)
    if ext == ".png" and not stem.endswith("-gemini"):
        return stem + "-gemini.png" in names
    if ext in (".wav", ".ogg") and not stem.endswith("-flow"):
        return stem + "-flow.ogg" in names or stem + "-flow.wav" in names
    return False


def plan(only):
    """(source, packed relative path, kind) for every file to ship."""
    out = []
    for dirpath, dirs, files in os.walk(SRC):
        dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS and not d.startswith("."))
        rel_dir = os.path.relpath(dirpath, SRC).replace(os.sep, "/")
        rel_dir = "" if rel_dir == "." else rel_dir + "/"
        if only and not rel_dir.startswith(only):
            continue
        names = set(files)
        for name in sorted(files):
            if name in SKIP_FILES or shadowed(dirpath, name, names):
                continue
            stem, ext = os.path.splitext(name)
            ext = ext.lower()
            kind = {".png": "image", ".wav": "sound"}.get(ext, "copy")
            out.append((os.path.join(dirpath, name), rel_dir + stem, ext, kind))
    return out


def pack_one(job):
    src, rel_stem, ext, kind, dest_root, profile = job
    image_opts, sound_fmt, sound_opts = PROFILES[profile]
    if kind == "image":
        rel = rel_stem + ".webp"
    elif kind == "sound":
        rel = rel_stem + "." + sound_fmt
    else:
        rel = rel_stem + ext
    dest = os.path.join(dest_root, rel)
    stamp = f"{PACK_VERSION}:{profile}:{digest(src)}"
    if os.path.exists(dest) and old.get(rel) == stamp:
        return rel, os.path.getsize(src), os.path.getsize(dest), stamp
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    tmp = dest + ".tmp"
    if kind == "image":
        from PIL import Image

        im = Image.open(src)
        im.load()
        if im.mode not in ("RGB", "RGBA"):
            im = im.convert("RGBA" if "A" in im.mode or "transparency" in im.info else "RGB")
        # a picture with an alpha channel that is opaque everywhere is kept without one
        if im.mode == "RGBA" and im.getchannel("A").getextrema() == (255, 255):
            im = im.convert("RGB")
        most = next((m for d, m in MAX_SIDE.items() if rel.startswith(d)), None)
        if most and max(im.size) > most:
            k = most / max(im.size)
            im = im.resize((round(im.size[0] * k), round(im.size[1] * k)), Image.LANCZOS)
        im.save(tmp, "WEBP", **image_opts)
        # a tiny picture can come out bigger as WebP; WebP lossless of it is never worse
        if os.path.getsize(tmp) > os.path.getsize(src) and not image_opts.get("lossless"):
            im.save(tmp, "WEBP", lossless=True, quality=100, method=5, exact=True)
    elif kind == "sound":
        import soundfile as sf

        data, rate = sf.read(src, always_2d=True)
        if sound_fmt == "flac":
            info = sf.info(src)
            subtype = "PCM_24" if info.subtype in ("PCM_24", "PCM_32", "FLOAT", "DOUBLE") else "PCM_16"
            sf.write(tmp, data, rate, format="FLAC", subtype=subtype)
        else:
            # written in blocks: libsndfile's Vorbis encoder crashes the process on Windows when
            # handed a long sound in one piece
            with sf.SoundFile(tmp, "w", rate, data.shape[1], format="OGG", **sound_opts) as out:
                for at in range(0, len(data), 1 << 15):
                    out.write(data[at : at + (1 << 15)])
    else:
        shutil.copyfile(src, tmp)
    os.replace(tmp, dest)
    return rel, os.path.getsize(src), os.path.getsize(dest), stamp


def digest(path):
    h = hashlib.blake2b(digest_size=16)
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


# what each packed file was packed from, as the last run left it (read in every worker)
old = {}


def load_manifest(path):
    global old
    try:
        with open(path, encoding="utf-8") as f:
            old = json.load(f)
    except (OSError, ValueError):
        old = {}


def main():
    args = sys.argv[1:]
    profile = args[0] if args and args[0] in PROFILES else "desktop"
    only = args[args.index("--only") + 1].strip("/") + "/" if "--only" in args else ""
    workers = int(os.environ.get("PACK_JOBS", "4"))
    dest_root = os.path.join(ROOT, "target", "pack", profile, "assets")
    manifest = os.path.join(ROOT, "target", "pack", profile, "manifest.json")
    load_manifest(manifest)
    jobs = [(src, rel, ext, kind, dest_root, profile) for src, rel, ext, kind in plan(only)]
    # the same name packed twice (a.png and a.webp both in the source) would hide one of them
    seen = {}
    for j in jobs:
        out = j[1] + {"image": ".webp", "sound": "." + PROFILES[profile][1]}.get(j[3], j[2])
        if out in seen:
            sys.exit(f"two files pack to {out}: {seen[out]} and {j[0]}")
        seen[out] = j[0]
    total_in = total_out = 0
    packed = []
    stamps = dict(old)
    with ProcessPoolExecutor(workers, initializer=load_manifest, initargs=(manifest,)) as pool:
        for i, (rel, a, b, stamp) in enumerate(pool.map(pack_one, jobs, chunksize=4)):
            packed.append(rel)
            stamps[rel] = stamp
            total_in += a
            total_out += b
            if i % 100 == 0:
                print(f"{i}/{len(jobs)} {total_in >> 20} MB -> {total_out >> 20} MB", flush=True)
    if not only:
        # whatever an earlier run packed that is no longer shipped goes
        keep = set(packed) | {"index.txt"}
        for dirpath, _, files in os.walk(dest_root):
            for name in files:
                rel = os.path.relpath(os.path.join(dirpath, name), dest_root).replace(os.sep, "/")
                if rel not in keep:
                    os.remove(os.path.join(dirpath, name))
        with open(os.path.join(dest_root, "index.txt"), "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(sorted(packed)) + "\n")
        stamps = {k: v for k, v in stamps.items() if k in keep}
    with open(manifest, "w", encoding="utf-8") as f:
        json.dump(stamps, f, indent=0, sort_keys=True)
    print(f"{profile}: {len(packed)} files, {total_in >> 20} MB -> {total_out >> 20} MB in {dest_root}")


if __name__ == "__main__":
    main()
