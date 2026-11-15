# Releases

## Packing the assets

`python -X utf8 tools/pack_assets.py desktop|mobile` packs `assets/` into
`target/pack/<profile>/assets`:

- pictures to lossy WebP (desktop q92, mobile q90; by eye the same as the sources), card art capped
  at 720 px (`MAX_SIDE`), sounds to Ogg Vorbis (written in blocks: libsndfile crashes on long
  sounds on Windows in one piece);
- files the game never loads are left out (a `name.png` shadowed by `name-gemini.png`, etc.);
- `index.txt` lists every packed file (Android cannot list a folder);
- `target/pack/<profile>/manifest.json` remembers what each file was packed from (content hash +
  `PACK_VERSION`), so a second run takes seconds and a cache restored from an older run redoes
  only what changed. Bump `PACK_VERSION` when the packing itself changes.

Desktop pack ~0.4 GB (was 1.5 GB with lossless WebP + FLAC).

## GitHub Actions — `.github/workflows/release.yml`

- Runs on a pushed tag `v*` or by hand: `gh workflow run release.yml --ref main -f tag=v0.1.N`.
  Prefer starting it by hand from `main`: only branch runs save caches; tag runs can only read
  `main`'s.
- Jobs: Windows (Inno Setup installer from `tools/installer.iss`), Linux and macOS (tar.gz with
  the Steam library beside the binary, rpath `$ORIGIN` / `@executable_path`), Android (cargo-apk,
  aarch64, signed with `tools/release.keystore`). All upload into the GitHub Release.
- Caches: the repository has 10 GB of cache in all and drops the oldest past that. The packed
  desktop assets are one cache shared by all three desktops (`enableCrossOsArchive`), restored
  from the newest older one by prefix (`pack-desktop-v2-`); Rust caches per platform
  (Swatinem/rust-cache). A run takes ~11 minutes warm.
- `rust-toolchain.toml` pins the toolchain; the Android target is added with `rustup target add`.

## Version and fingerprint

The game's version comes from the release tag (`GAME_VERSION`, see `crates/game/build.rs`); the
updater compares it. A LAN party only lets in a player with the same build fingerprint (version
+ every rule file), so two builds never deal one run two ways.

## Pushing

The repository is large: a first push of a long history goes in chunks (GitHub refuses one push
over 2 GB).
