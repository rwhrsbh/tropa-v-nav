# Sound and voice: how they are made and taken into the game

Texts of every spoken line: `docs/prompts/voice/` (`story.<lang>.txt`, `barks.<lang>.txt`,
`omens.ru-old.txt`, casting in `barks.voices.txt`). Music and sound prompts:
[`prompts/02_music.md`](prompts/02_music.md).

## Languages and manner — never one style for all

Three spoken languages: `ru-old` (Old Russian), `ru`, `en`. Each is recorded in a run of its own,
with a style for that language alone: one style across languages once ruined ~240 clips (modern
Russian read the old way, English with no accent). An English persona text bleeds an English
accent into ru-old: the ru-old style says the speaker is a native Russian with no foreign accent
(`STYLE` in `tools/tts_api.py`). Before any mass run with a new voice, model or style, record 2–3
clips and let the user listen.

## Narrator (story pages, book secrets, omens)

- Model `gemini-3.1-flash-tts-preview`, speaker **Algenib**, the scene prompt approved by the user
  (in `tools/tts_api.py` / `docs/prompts/voice/story.<lang>.txt`): a grim old village storyteller,
  low hoarse voice, same loudness from first word to last, never a whisper, calm menace.
- Old Russian text is fed with the stressed vowel of each word in CAPITALS and no «ё»; the style
  asks for the church manner (every vowel as written, unstressed O stays O).
- Through the API: `tools/tts_story.py <lang> [ids]`, `tools/tts_omens.py <lang> [count] [ids]`.
  Keys in `.secrets/gemini_keys.txt`; each model has its own quota per key, the scripts fall back
  across keys and models.
- Through the AI Studio page (aistudio.google.com/generate-speech): set textareas by their
  aria-labels («Scene», «Speech block text») with the native setter + `input` event; download only
  after the Run button is enabled again (audio streams; an early save is cut short). Quota per
  account dies after a few dozen runs: close the notice with its X only.

## Battle lines (barks)

- `tools/tts_api.py <who|all> <lang> [count]` records every missing line of a speaker through the
  API with that speaker's voice and the language's style. Five takes per line; the game picks a
  random one, never the same twice running.
- Page helper alternative: `tools/barkpage.py` → `_inbox/barkpage.js` (`tropaStyle`,
  `tropaRecord`), lite model `gemini-3.8-flash-lite-tts` with the style in the Style box.
- Read each line as the listener hears it: "Я здесь, уродина" sounded like the bogatyr calling
  himself ugly.

## Checking every clip

- `tools/voice_in.py <lang> <key>` / `--named`: a local Whisper (`tools/hear.py`, GPU) scores the
  clip against its line and prints the loudness by fifths; bad clips are set aside, good ones are
  levelled into `assets/voice/<lang>/<key>.wav`.
- `tools/find_repeats.py <lang> <bark|story|omen>` finds clips where a line is said twice;
  `tools/redo_doubled.py` moves them to `_inbox/voice/doubled` and records them again.

## Sound effects and ambience

- ElevenLabs sound generation: `tools/sfx_api.py [name ...] [--takes 3]` (key in
  `.secrets/elevenlabs_key.txt`). Takes land in `_inbox/sfx/`, the first goes in as
  `assets/sfx/<name>-flow.wav`; `--pick <name> <take>` swaps another take in; `--ambience` makes
  each act's battle ambience. Prompts are the `SOUNDS` table in the script.
- Each status has its own sound `status_<kind>`; cues are wired in `crates/game/src/audio.rs`.

## Music

- Stand-ins: `tools/gen_music.py` synthesises looping tracks on the leitmotif A–C–D–E–C.
- Generated music goes in as `assets/music/<name>-flow.ogg`; battle music is three layers of
  equal length (`_l1` drums/bass, `_l2` melody, `_l3` choir/heavy drums) that the game fades.
  `tools/music_in.py` takes downloads in.
