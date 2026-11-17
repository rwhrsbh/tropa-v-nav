# AGENTS.md — «Тропа в Навь»

Rust/Bevy 0.20 co-op deckbuilder. This file only points the way: the rules and recipes live in
`docs/`, read the one you need when you need it.

## Contents

- [How we work](docs/README.md)
  - [Talking and writing](docs/README.md#talking-and-writing)
  - [Git](docs/README.md#git)
  - [Building and testing](docs/README.md#building-and-testing)
  - [What never to touch](docs/README.md#what-never-to-touch)
  - [Before calling any picture or sound done](docs/README.md#before-calling-any-picture-or-sound-done)
- [Open work](docs/todo.md)
  - [Gorynych as one body with three heads](docs/todo.md#gorynych-as-one-body-with-three-heads)
- [Pictures](docs/art_pipeline.md)
  - [Where pictures are painted](docs/art_pipeline.md#where-pictures-are-painted)
  - [Style](docs/art_pipeline.md#style)
  - [Keying and sizes](docs/art_pipeline.md#keying-and-sizes)
  - [Foe poses](docs/art_pipeline.md#foe-poses)
  - [Rig parts and corpses (the dismemberment)](docs/art_pipeline.md#rig-parts-and-corpses-the-dismemberment)
  - [Maimed heroes on the Edge](docs/art_pipeline.md#maimed-heroes-on-the-edge)
  - [Other kinds](docs/art_pipeline.md#other-kinds)
- [Sound and voice](docs/audio_pipeline.md)
  - [Languages and manner — never one style for all](docs/audio_pipeline.md#languages-and-manner--never-one-style-for-all)
  - [Narrator (story pages, book secrets, omens)](docs/audio_pipeline.md#narrator-story-pages-book-secrets-omens)
  - [Battle lines (barks)](docs/audio_pipeline.md#battle-lines-barks)
  - [Checking every clip](docs/audio_pipeline.md#checking-every-clip)
  - [Sound effects and ambience](docs/audio_pipeline.md#sound-effects-and-ambience)
  - [Music](docs/audio_pipeline.md#music)
- [Texts and translations](docs/i18n.md)
  - [Languages and files](docs/i18n.md#languages-and-files)
  - [Fluent pitfalls](docs/i18n.md#fluent-pitfalls)
  - [Generated text](docs/i18n.md#generated-text)
  - [Voice text](docs/i18n.md#voice-text)
- [Releases](docs/release.md)
  - [Packing the assets](docs/release.md#packing-the-assets)
  - [GitHub Actions — `.github/workflows/release.yml`](docs/release.md#github-actions--githubworkflowsreleaseyml)
  - [Version and fingerprint](docs/release.md#version-and-fingerprint)
  - [Pushing](docs/release.md#pushing)
- [Balance analytics](docs/telemetry_and_analytics.md)
  - [What is recorded](docs/telemetry_and_analytics.md#what-is-recorded)
  - [Where it goes](docs/telemetry_and_analytics.md#where-it-goes)
  - [The sheet](docs/telemetry_and_analytics.md#the-sheet)
  - [In the game](docs/telemetry_and_analytics.md#in-the-game)
- [Every tool in tools/](docs/tools.md)
  - [Foes: rig parts, corpses, poses](docs/tools.md#foes-rig-parts-corpses-poses)
  - [Pictures: cards, relics, icons, story](docs/tools.md#pictures-cards-relics-icons-story)
  - [Voice](docs/tools.md#voice)
  - [Sound and music](docs/tools.md#sound-and-music)
  - [Text](docs/tools.md#text)
  - [Release and analytics](docs/tools.md#release-and-analytics)
  - [Undocumented or other](docs/tools.md#undocumented-or-other)
- [The book's secrets](docs/book_secrets.md)
  - [Акт I — «Лес». Страницы 5–6](docs/book_secrets.md#акт-i--лес-страницы-56)
  - [Акт II — «Деревня». Страницы 10–11](docs/book_secrets.md#акт-ii--деревня-страницы-1011)
  - [Акт III — «Навь». Страницы 15–16](docs/book_secrets.md#акт-iii--навь-страницы-1516)
  - [Акт IV — «Кощей». Страницы 20–21](docs/book_secrets.md#акт-iv--кощей-страницы-2021)
  - [Акт IV — «Остров Буян». Страницы 24–25](docs/book_secrets.md#акт-iv--остров-буян-страницы-2425)
  - [Проклятие «Порча книги»](docs/book_secrets.md#проклятие-порча-книги)
- Prompts, word for word: [docs/prompts/](docs/prompts/) — foes `01_enemies.md`, music `02_music.md`,
  cards `03_cards.md`, status icons `04_status_icons.md`, potions and omens `05_potions_omens.md`,
  places `06_locations.md`, voice texts and casting `voice/`
- Game design: `docs/prompts/Тропа_в_Навь_—_дизайн_документ_и_промпты.md`

Code map: `crates/core` (rules, no graphics: combat, run, bot, content data), `crates/game` (Bevy
client: battle, map, book, story, net, UI), `crates/net` (party protocol), `crates/android`,
`assets/data/*.ron` (all content), `assets/i18n/<lang>/*.ftl` (all text), `tools/` (asset
pipelines, each script explains itself in its docstring).
