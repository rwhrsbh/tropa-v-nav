# How we work on «Тропа в Навь»

The standing rules, gathered from the whole of the work so far. Each pipeline has its own page:
[art](art_pipeline.md), [sound and voice](audio_pipeline.md), [texts](i18n.md),
[releases](release.md).

## Talking and writing

- Replies to the user in **Russian**. Code, comments, commit messages, docs for agents: English.
- Commit messages say what the change does for the game, in plain words, and end with
  `Co-Authored-By: <the model> <noreply@anthropic.com>`.
- Comments and docstrings say *why* and *what for*, in full sentences; they stay when code is
  edited around them.

## Git

- Work on `main`; `master` is kept equal to it (`git push origin main` and `git push origin
  main:master`). Pushed to https://github.com/rwhrsbh/tropa-v-nav.
- Others push to `main` too (Seeeh9 and their agents). Before pushing: `git fetch`, merge, and
  on a conflict keep whichever side is done better, *by the user's word* where the user has
  spoken (for example Kot Bayun's post and Korochun's numbers are ours, the Pike's flight theirs).
- Never `git add` the user's own untracked files: `tools/*lyrics*`, `oxxxy_*.json`,
  `transcription_results.json`, `tools/__pycache__`, `tools/flow_bridge/`, root `_*` folders and
  pictures, `assets/assets/`, `*~` files, `ogneya_ref.png`, `test_bayun*`. Add files by name.
- After a crash of the PC: look for zeroed files (`main.rs`, android `lib.rs` were zeroed twice)
  and stale `.git/*.lock` files; a corrupt index is rebuilt with `rm .git/index && git reset`.
- Python edits must keep line endings: `open(p, 'w', encoding='utf-8', newline='')`. Check
  `git diff --stat` after scripted edits: hundreds of changed lines mean the endings flipped.
- In Git Bash a heredoc fed to `python -` loses backslashes (`'\\'` arrives as `'\'`): put such
  scripts in a file. `cmd /c` arguments need `MSYS_NO_PATHCONV=1`.

## Building and testing

- `cargo build -p tropa --features dev` for a dev build; tests: `cargo test -p nav_core -p tropa
  -p nav_net`. Every rule change gets a test in `crates/core/tests/`.
- Local release builds: Windows only, and only when asked. Every platform builds in CI
  ([releases](release.md)).
- Test runs never take the user's screen: with `TROPA_SHOT` set the window opens hidden, off-screen,
  unfocused. Never kill the user's game; only processes you started. If `target/debug/tropa.exe`
  is locked by their running game, use `cargo check`.
- Dev switches are listed at the top of `crates/game/src/dev.rs` (`TROPA_SCREEN`, `TROPA_FIGHT`,
  `TROPA_HERO`, `TROPA_HAND`, `TROPA_ROBOT` mouse scripts in stage pixels, `TROPA_SHOT[_AT,_COUNT,
  _EVERY]`, `TROPA_STORY`, `TROPA_LOSE` to fake a lost connection...). Look at every change in the
  real game with screenshots before calling it done, and show the user the sheet.
- When the user asks to test something themselves, launch it and wait for their word before
  tuning further.

## What never to touch

- Backdrops `assets/bg/*` are never repainted or edited: places for figures go in
  `assets/data/stands.ron`.
- Keys in `.secrets/` (Gemini, ElevenLabs) are never printed, logged or committed.
- In browsers: no paid buttons, no plans or billing pages, no reCAPTCHA, no Save-as dialogs.
- A Flow prompt refused by a content filter is skipped, never reworded to slip past it
  (Yemelya once tripped the minors filter; he is drawn as an adult man in every prompt).

## Before calling any picture or sound done

- Sheet the old beside the new, mark what changed, look at every file by eye: a "cleanup" script
  once ate the Ovinnik's smoke and called it clean. Repaints keep the figure's bounds and look.
- Voice: the user hears 2–3 test clips before any mass run with a new voice, model or style.
