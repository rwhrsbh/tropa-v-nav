# AGENTS.md — «Тропа в Навь»

Rust/Bevy 0.20 co-op deckbuilder. This file only points the way: the rules and recipes live in
`docs/`, read the one you need when you need it.

- **Start here:** [`docs/README.md`](docs/README.md) — how we work: language, commits, git, builds,
  testing, what never to touch.
- **Pictures** (Flow, Gemini, keying, rig parts, corpses, poses, icons, backdrops):
  [`docs/art_pipeline.md`](docs/art_pipeline.md), prompts in [`docs/prompts/`](docs/prompts/).
- **Sound and voice** (narrator, battle lines, omens, sound effects, music, Whisper checks):
  [`docs/audio_pipeline.md`](docs/audio_pipeline.md).
- **Texts and translations** (ru, ru-old, en; Fluent keys; generated enemy-move text):
  [`docs/i18n.md`](docs/i18n.md).
- **Releases** (asset packing, GitHub Actions, caches, Android signing, installer):
  [`docs/release.md`](docs/release.md).
- **Game design notes:** `docs/prompts/Тропа_в_Навь_—_дизайн_документ_и_промпты.md`,
  `docs/book_secrets.md`, `docs/telemetry_and_analytics.md`.

Code map: `crates/core` (rules, no graphics: combat, run, bot, content data), `crates/game` (Bevy
client: battle, map, book, story, net, UI), `crates/net` (party protocol), `crates/android`,
`assets/data/*.ron` (all content), `assets/i18n/<lang>/*.ftl` (all text), `tools/` (asset
pipelines, each script explains itself in its docstring).
