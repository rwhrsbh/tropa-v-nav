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

The language styles, word for word (added after the speaker's persona from
`docs/prompts/voice/barks.voices.txt`):

- **ru-old:** "The line is in Old Russian, read in the solemn old manner: every vowel as written,
  unstressed O stays a round O, E never read as YO; a capital letter inside a word marks its
  stressed vowel. The speaker is a native Russian: Russian pronunciation through and through, with
  no English or other foreign accent at all."
- **ru:** "The line is in plain modern Russian: ordinary present-day pronunciation and natural
  stress, nothing archaic or churchly."
- **en:** "The line is in English, spoken by a Slav: a clear, noticeable Russian accent, rolled R,
  heavy consonants."

### Who speaks: voice and persona of every character

(The same as `docs/prompts/voice/barks.voices.txt`, which the tools read; change both together.)

| who | voice | persona |
|---|---|---|
| bogatyr | Alnilam | A huge Slavic warrior knight in battle. Deep, strong, booming chest voice of a man of forty; calm, scornful confidence; short soldier's phrases spoken firmly and loudly. Not theatrical. |
| vorozheya | Gacrux | A woman: a middle-aged village witch who reads fate in bones and cards. Strictly a female voice, never male. Low, husky, knowing woman's voice; cold and mocking; she speaks in a quiet hiss full of contempt. Not a whisper: every word clear. |
| volhv | Orus | An old pagan priest of the thunder god in battle. Grave, resonant, commanding elder's voice, slow and solemn as a prayer, with iron under it. |
| volkolak | Fenrir | A man half turned into a wolf, mid-fight. Rough, hungry, growling voice through bared teeth, panting between words, barely holding back the beast. Every word still understandable. |
| znaharka | Sulafat | A woman: a kindly but grim old village healer, about seventy. Strictly a female voice, never male. Warm, cracked, folksy grandmother's voice; dry black humour; brisk and practical. |
| skomoroh | Puck | A wandering Slavic jester and buffoon performing even in battle. Quick, nasal, mocking, theatrical showman's voice; he laughs at death; loud and bright. |
| emelya | Umbriel | A lazy, sleepy young village fool lying on a warm stove. Slow, drawling, yawning, good-natured voice, thick with sleep; he mumbles and never hurries; even in pain he sounds mostly put out, not afraid. |
| leshy | Algieba | The ancient forest spirit Leshy. Enormous, slow, creaking, deep voice like an old tree groaning in the wind; inhuman and patient. |
| bolotnik | Enceladus | The swamp master Bolotnik. Thick, wet, gurgling, drawling voice rising from under black water; slow and hungry. |
| mara | Kore | A woman: Mara, the goddess of winter and death. Strictly a female voice, never male. Cold, flat, clear, icy woman's voice; utterly calm and merciless; every word like frost. |
| koschei | Charon | Koschei the Deathless, an ancient skeletal king. Dry, rasping, aristocratic old voice, amused and bored, slow; a voice that has not breathed for a thousand years. |

### How a line was asked, and why two takes of one line can sound different

The full prompt is always **persona + language style + the line**. Two ways were used:

- **Through the API** (`tools/tts_api.py`, most of the takes): one text,
  `<persona> <style> Say only this line, once, in character: <line>`; the 3.8 models get
  the same laid out in two parts, a `## Style:` heading over `<persona> <style>` and a
  `## Transcript:` heading over the line, instead.
- **On the AI Studio page** (`tools/barkpage.py`, the bogatyr's first takes): the Style box holds
  `<persona> <style> Speak only the line, once.` (the ru style there adds "The capital letters are
  only the start of sentences."), the line goes in the speech block alone, model
  `gemini-3.1-flash-tts-preview`.

The script takes the first model with quota left (`MODELS` in `tools/tts_api.py`:
gemini-3.1-flash-tts-preview, gemini-3.8-flash-tts, gemini-2.5-pro-preview-tts,
gemini-2.5-flash-preview-tts, gemini-3.8-flash-lite-tts), so the takes of one speaker were made by
different models on different days, and each model makes the same voice sound a little different.
The ru-old style got its last sentence ("The speaker is a native Russian… no foreign accent") on
2026-11-09; takes made before that lack it. When one take is missing, record it with the model its
siblings came from and let the user hear it beside them before it goes in.

A battle line is asked as `<persona> <style> Say only this line, once, in character: <line>`; the
3.8 models take it laid out as `## Style:` / `## Transcript:`. The narrator's scene for each
language is the comment block at the top of `docs/prompts/voice/story.<lang>.txt`; a page is asked
as `<scene>` + `Read this aloud, once, exactly as written:` + the text.

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
