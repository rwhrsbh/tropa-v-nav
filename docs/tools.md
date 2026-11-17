# Tools

Every script in `tools/`, what it is for and how it is run, taken from its own docstring
(`python tools/make_tools_doc.py` writes this page). The step-by-step recipes that string them
together are in [art_pipeline.md](art_pipeline.md), [audio_pipeline.md](audio_pipeline.md),
[i18n.md](i18n.md) and [release.md](release.md). Run Python tools from the repository root with
`python -X utf8 tools/<name>.py`.

## Foes: rig parts, corpses, poses

### `flow_rig_cmds.py`

Prints the Browser MCP command block that paints one foe's rig edits in Google Flow.

```
python tools/flow_rig_cmds.py <id> "<who>" "<head>" [<pronoun>]
```

<who> describes the foe ("gaunt black hound of the dead"), <head> what the cyan mask and the
beheading take ("its whole head with both ears, down to the iron collar"), <pronoun> is its/his/
her. The block uploads `_inbox/flow_up/rig_<id>.png` (the resting picture on a green square),
paints the colour mask, the headless body, the corpse and the headless corpse, and saves them as
`_inbox/flow_up/<id>_{mask,body,dead,deadhl}.png` for `tools/rig_in.py`. Paste the output into
one `browser` call (it runs about four minutes).

### `rig_in.py`

Takes a foe's four Flow edits into the game: its head and headless body for the rig, and its
two corpses.

```
python tools/rig_in.py <id> <mask> <body> <dead> <dead_headless>
```

<mask> is the resting picture with the whole head painted flat cyan, <body> the same picture with
the head cut off and a neck stump, <dead> the foe lying dead, <dead_headless> that corpse with its
head gone. The first two go through tools/rigparts.py (head and body, laid over the resting
picture, which is never changed); it prints the line for `rig::FIGURES`. The corpses are keyed by
flooding the background from the edges (tools/key_flood.py) and laid on a wide sheet as the older
foes' are: 3:2, the body as long as 0.88 of a standing foe's sheet is tall, on the same foot line;
the headless one at the corpse's own scale. A preview sheet lands in target/rig_<id>.png.

### `rigparts.py`

Split a painted figure into its body and moving parts.

python tools/rigparts.py <original.png> <parts_removed.jpg> <colour_mask.jpg> <out_dir> <name> <preview.png>

The colour mask is the original with each part painted flat: magenta the weapon arm, cyan the
head, yellow the shield or off arm, green a second arm, blue a tail or cloak.
Writes <name>_body.png (the removed-parts edit under the parts, the original elsewhere) and
<name>_<part>.png for each part found, all the size of the original.

### `rig_fill.py`

Gives back to a foe's rig parts what fell between them when it was cut.

```
python tools/rig_fill.py [<id> ...] [--dry]
```

A head and a body cut from Flow's colour mask and beheaded picture often miss a rim of the resting
picture: the tip of a snout the mask did not paint, a flame, a strand of hair, the outline Flow ate
away. In the game those pixels are simply gone (Kot Bayun's muzzle came out cut flat). Every pixel
of the resting picture that no part holds goes to the nearer of the head and the body (arm and
shield parts count as holding theirs), copied from the resting picture. With no ids, every foe
with a `_head` and a `_body`. `--dry` only writes target/rig_fill.png: each foe before (missing in
red) and after, to look at first.

### `key_flood.py`

Keys a figure painted on flat green (or any flat colour) out of its picture by flooding the
background in from the picture's edges: whatever the flood cannot reach stays solid, so green
glints inside fur, eyes or cloth never turn into holes, as a plain colour key makes them. Gaps
of background shut in by the figure (between legs, inside an arm's crook) are found by size: a
patch of the background's own colour larger than `gap` pixels is background too.

```
from key_flood import key_flood
rgba = key_flood(Image.open("flow.jpg"))
```

### `flow_pose_cmds.py`

Prints the Browser MCP command block that paints a foe's missing fight poses in Google Flow.

```
python tools/flow_pose_cmds.py <id> [strike|hurt|guard|cast ...]
```

With no poses named, it paints every pose the foe lacks for its moves: `_hurt` always, `_strike`
for attacks, `_guard` for defending, `_cast` for buffs, curses, summons and specials. The foe is
described by the words of its rig prompts (`_inbox/flow_up/cmd_<id>.txt`) or, failing that, by its
line in docs/prompts/01_enemies.md. The resting picture on a green square,
`_inbox/flow_up/rig_<id>.png` (made if missing), is attached as the look only and each pose is
asked as a new picture; they land as `_inbox/flow_up/<id>_<pose>.png`, to be taken in with
`python tools/pose_in.py _inbox/flow_up/<id>_<pose>.png <id> <id>_<pose> _inbox/flow_up/rig_<id>.png`
(the green square is the scale source: it holds the resting picture at its own size). Paste the output into one
`browser` call (about a minute per pose).

### `pose_batch_in.py`

Takes in a batch of poses painted by `tools/flow_pose_cmds.py --batch`.

```
python tools/pose_batch_in.py [<n> ...]
```

Reads `_inbox/flow_up/batch/titles.txt` (the tiles' own titles, which say the pose; the fetch
prints them) or else `batch.txt` (tile number, foe, pose) and takes every pose tile in with
tools/pose_in.py, skipping uploads and the tile numbers given (the ones that came out wrong, to be
asked again). Writes target/batch.png, every tile with its label, to check by eye first.

### `pose_in.py`

Takes a pose painted in Flow on flat green into the game at the same scale as its resting picture.

```
python tools/pose_in.py <flow picture> <rest stem> <out stem> [<scale source>]
```

<rest stem> is the resting picture in assets/enemies (e.g. `kot_bayun_ground`), <out stem> the
pose to write (e.g. `kot_bayun_ground_strike`). The figure is keyed by flooding the background in
from the edges (tools/key_flood.py), scaled as the resting picture's own source was (the Flow
picture it was cut from, <scale source>, by default `_inbox/flow_up/<rest stem>_src.png`), and
stood on the same foot line. A pose wider than the resting picture gets a wider sheet; the game
draws it at its own width (`rig::fit`).

### `foe_rig.py`

Makes a foe's game sprites from one picture painted on green in Google Flow.

```
python tools/foe_rig.py <picture> <id> <size> <neck>
```

<size> is the sheet's side or WxH (512x768 for a common foe, 1024 for an elite or a boss);
<neck> is how far down the figure, 0..1, the head comes off; a beast on all fours facing left takes
<x>,<y> instead: the head is what lies left of x and above y, both as shares of the figure's box;
<l>,<r>,<t>,<b> takes just that box, for a head with wings or arms around it. Writes into assets/enemies/:
  <id>-gemini.png                 the whole figure, keyed and planted on the baseline
  <id>_head / <id>_body           cut at the neck with a soft seam, for the rig's nod and the killing blow
  <id>_dead / <id>_dead_headless  the figure laid on the ground
and prints the rig.rs line with the head joint.

### `cutout.py`

Turns a generated character on a green screen into a game sprite.

```
python tools/cutout.py <source.png> <assets-relative stem> <width> <height> [--foot 0.957]
```

Keys out the green, removes the green spill from edge pixels, trims to the
figure, scales it into width x height and plants the feet on the baseline
(y = 980 of 1024 for heroes, as the asset spec asks). Writes `<stem>-gemini.png`.

### `trim_tail.py`

Cuts the noise some speech models leave at the ends of a clip.

```
python tools/trim_tail.py <file.wav|folder> [...]   # rewrites the clips that have it
```

The 3.8 speech models end a clip with about a tenth of a second of loud hiss straight after the
silence that follows the last word, and a click at the start: these were the wav header and
trailing chunks, written as sound before tts_api.py unwrapped the 3.8 models' wav replies.
The hiss is a last stretch many times louder than the quiet before it; the click is a first five
milliseconds far louder than what follows.

## Pictures: cards, relics, icons, story

### `flowart.py`

Turns pictures downloaded from Google Flow into card illustrations or relic icons.

```
python tools/flowart.py <card id> [...]            # 1000x750 -> assets/cards/<id>-gemini.png
python tools/flowart.py --relic <relic id> [...]   # 256x256   -> assets/relics/<id>-gemini.png
python tools/flowart.py --story <page id> [...]    # 1920x1080 -> assets/story/<id>-gemini.png
python tools/flowart.py --list                     # what is waiting, oldest first
python tools/flowart.py --match [--relic] <id> [...] # pair waiting files with these ids by their titles
python tools/flowart.py --packs                    # take in every tropa-pack-*.bin and tropa-<kind>-<id>.jpg
```

A pack is what the page helper `flowPack` saves: several full-size pictures in one file, each
with its kind and id, because the browser lets a page save only one file at a time.

Flow names a download `<Some_title>_<yyyymmddhhmmss>.jpg`; only such files, newer than a day,
are taken, in the order they arrived. `-` throws a picture away. Originals move to `_inbox/`.

### `flow_next.py`

Prints one message for the Google Flow agent asking for the next N unpainted cards.

```
python tools/flow_next.py [N=4] [--relics]
```

Cards come from docs/prompts/plots/*.json in file order, skipping those that already have
assets/cards/<id>-gemini.png. The ids, in the order asked, go to stderr: the pictures come
back in that order, and that is the order to give tools/flowart.py.

### `flow_page.js`

Helpers for a Google Flow project tab, pasted into the page. They work through its ordinary
interface: the page itself sends every request, and nothing here touches its session or its checks.

repaint(change)      types the "same picture, only this change" prompt (the picture is pasted first)
cut(side), second(side)   the prompts that take an arm off cleanly: one arm from the whole figure,
then the other from that result. Asking for both at once comes out badly.
grabMany([0, 2, 4])  downloads those tiles at 1K, newest first; refs lie between results, so check by eye

A paste of the reference, repaint(...) and Return each take a few seconds; several can be sent without
waiting for the one before. Downloads land in Downloads under Flow's own names, in the order asked.

### `gen_art.py`

Procedural fallback art for «Тропа в Навь».

Everything here is a stand-in that the game uses only until a generated
`<name>-gemini.png` sits next to it. Shapes are blocked in flat, lit from the
upper left with a cold rim light (art bible, section 9), then repainted with
brush strokes so nothing looks like a vector drawing.

```
python tools/gen_art.py [--only bg|heroes|enemies|cards|allcards] [--force]
```

### `cardart.py`

Turns freshly downloaded Gemini pictures into card illustrations.

```
python tools/cardart.py <card id> [<card id> ...]
```

Takes the waiting downloads in the order they arrived (see collect.py), crops each to 4:3
around its centre, scales it to 1000x750 and writes assets/cards/<id>-gemini.png.
Use `-` for a picture to throw away.

### `cardsheet.py`

Contact sheet of card illustrations, three to a row.

python tools/cardsheet.py <out.png> <card id> [...]

### `cards3.py`

The third card set: fills every hero up to 75 cards + 5 co-op ones.

The tables below are the source; this script writes
assets/data/cards/set3.ron and assets/i18n/{ru,en}/cards3.ftl from them.
Card text is not written here: the game composes it from the effects.

```
python tools/cards3.py
```

A row is
```
c(id, kind, rarity, cost, target, effects, base, up, ru, en, kw="", upcost=None)
```
kind:    A attack, S skill, H herb, P power (effects are then "Trigger|effects")
rarity:  C common, U uncommon, R rare
target:  E enemy, H any hero, A ally, D downed, - none
effects: RON with {0} {1}.. for the numbers; `base` and `up` fill them in
kw:      letters: x exhaust, r retain, e ethereal, c co-op, X x-cost
upcost:  the upgrade changes the cost instead of (or as well as) the numbers
beast:   the werewolf's other side: (kind, target, effects, base, up)

Balance (design doc, section 4): 1 energy = 6 damage to one, 5 block, 4 damage to all,
2 stacks of a debuff; a card drawn = 0.5; uncommon +15%, rare +30% or a unique effect;
upgrade = +30-50% or -1 cost.

### `cards4.py`

The fourth card set: ten cards a hero, opened by hero levels 3 to 10.

They bring new ways to build a deck: throwing cards away on purpose (and cards that
pay off when thrown), burning them, calling them back from the discard pile, and
payoffs for each hero's own mechanic.

```
python -X utf8 tools/cards4.py
```

writes assets/data/cards/set4.ron and assets/i18n/{ru,en}/cards4.ftl. A row is
```
c(id, kind, rarity, cost, target, effects, base, up, ru, en, level,
  kw="", upcost=None, disc=None, upfx=None)
```
as in tools/cards3.py, plus
level:  hero level that opens the card in rewards
disc:   what the card does when a card or a power discards it ({0}.. as in effects)
upfx:   the upgrade's effects when they differ in shape, not only in numbers

### `relicart.py`

Turns freshly downloaded Gemini pictures into relic icons.

```
python tools/relicart.py <relic id> [<relic id> ...]
```

Takes the waiting downloads in the order they arrived (see collect.py), crops each to a
centred square, scales it to 256x256 and writes assets/relics/<id>-gemini.png.
Use `-` for a picture to throw away.

### `relic_prompts.py`

Writes docs/prompts/plots/relics.json: one full image prompt per relic.

```
python tools/relic_prompts.py
```

The subject is the relic's English name unless WHAT says more exactly what to paint.
The file is marked `_raw`: the browser line sends these prompts as they are.

### `intentart.py`

Turns emblems downloaded from Google Flow into enemy intent icons.

```
python tools/intentart.py <intent kind> [...]   # in the order the pictures arrived
python tools/intentart.py --again <dir>         # redo from kept originals <dir>/<kind>_*.jpg
python tools/intentart.py --into status <name> [...]   # status icons, assets/status/
python tools/intentart.py --ink --into map <kind> [...]  # ink map symbols drawn on white
```


Each picture is painted on flat black; the black is keyed out to transparency by brightness,
so the emblem floats over any backdrop. The result is 128x128: assets/intents/<kind>-gemini.png.
Kinds are the `IntentKind` names in crates/core (Attack, AttackAll, Defend, ...); `-` throws a
picture away. Originals are kept as `_inbox/intents/<kind>_f1.jpg`.

### `icon_api.py`

Paints the game's app icon through the Gemini API (an image model), several takes.

```
python -X utf8 tools/icon_api.py [takes]
```

Takes land in `_inbox/icon/take_<n>.png`; the chosen one is made into every size the platforms want
by `tools/make_icons.py`.

### `omenart.py`

Takes omen pictures downloaded from Google Flow into the game.

```
python tools/omenart.py <omen id> [...]   # in the order the pictures arrived
```

Each picture is cropped square from its middle and saved at 768x768 as
assets/omens/<id>-gemini.png, the picture framed beside the omen's words (run_ui.rs
`omen_picture`). `-` throws a picture away. Originals are kept as `_inbox/omens/<id>.jpg`.

### `placeart.py`

Turns backdrops downloaded from Google Flow into battle places.

```
python tools/placeart.py <sheet.png> <enemy id> [...]          # -> assets/bg/at_<id>-gemini.png, 1920x1080
python tools/placeart.py --story <sheet.png> <page id> [...]   # -> assets/story/<id>-gemini.png, 1920x1080
```

Takes the waiting Flow downloads in the order they arrived, one per id (`-` throws a picture
away), and writes a labelled contact sheet for checking by eye. Originals move to `_inbox/`.

### `bookart.py`

Takes the book's textures from Downloads (Flow, 2K) into assets/book/.

```
python tools/bookart.py
```

Pictures are told apart by the words Flow puts in their names. The cover is cut to the book
itself (Flow paints a little white around it); the candle flame is keyed out of its black;
the tileable ones are made truly seamless (a half-offset copy is blended over the seams, as
Flow's "seamless" seldom quite is). Originals move to _inbox/book/.

### `inkcut.py`

Cut the parchment out from under painted ornaments, so they print onto the book's own page.

```
python tools/inkcut.py            # frame and headpiece
python tools/inkcut.py <stem>...  # e.g. book/frame
```

Reads assets/<stem>-gemini.png and writes assets/<stem>-cut.png with alpha: pixels close to the
parchment (estimated from the picture's border) turn clear, the ink stays. The game prefers the cut
file when it is there.

### `labelsheet.py`

Contact sheet with each picture's id and English name written under it, for checking by eye.

python tools/labelsheet.py <out.png> cards|relics <id> [...]

### `sheet.py`

Contact sheet of cut-out sprites on the game's dark ground, for checking a batch by eye.

python tools/sheet.py <out.png> <assets-relative stem> [...]

### `collect.py`

Moves freshly downloaded Gemini images into `_inbox/` under their asset names.

```
python tools/collect.py <name> [<name> ...]
```

The newest N `Gemini_Generated_Image_*` files in the Downloads folder are taken
in the order they arrived and renamed to `_inbox/<name>_<k>.jpg`, where k is the
first free take number. Use `-` for a name to skip (delete) that download.
Only files matching that pattern and newer than a day are touched.

### `wave_prompts.py`

Prints the Google Flow agent messages for the new wave of foes (docs/prompts/plots/wave.json).

```
python tools/wave_prompts.py            # every batch, one message per line, ids to stderr
python tools/wave_prompts.py --json     # [{"ids": [...], "text": "..."}] for pasting into the page
```

Batches never mix kinds, so each message carries one style tail: foes on green, icons on black,
cards as 4:3 scenes. Things that already have their picture are left out.

### `old_letters_font.py`

Adds the pre-1918 letters (yat, fita, izhitsa) that Ruslan Display lacks, built from its own
glyphs, and renames the font as the OFL asks of a changed one:
```
python tools/old_letters_font.py assets/fonts/RuslanDisplay-Regular.ttf assets/fonts/NavDisplay-Regular.ttf
```

## Voice

### `tts_api.py`

Records battle lines through the Gemini API instead of the AI Studio page.

```
python tools/tts_api.py <who|all> <ru-old|ru|en|all> [count]
```

API keys are read from `.secrets/gemini_keys.txt` (one per line; the folder is gitignored). Every
speech model has its own quota on every key: when one pair runs out the next is used (MODELS order). Every line still missing for that speaker and language
is spoken with the speaker's voice and a style for that language alone (see STYLE), then checked
and levelled by voice_in.py's `take`, exactly like a clip from the page.

### `tts_story.py`

Records story pages and book secrets through the Gemini API.

python -X utf8 tools/tts_story.py <ru-old|ru|en> [page_ids ...]

### `tts_omens.py`

Records the storyteller reading the omens (знамения) aloud, through the Gemini API.

```
python -X utf8 tools/tts_omens.py <ru-old|ru|en> [count] [id ...]
```

Each omen is read as its title and its text: `assets/voice/<lang>/omen-<id>.wav`, played as the
omen opens. The voice is the tale's narrator (speaker Algenib, gemini-3.1-flash-tts-preview, the
scene of `docs/prompts/voice/story.<lang>.txt`), each language with its own scene. The old speech
reads `docs/prompts/voice/omens.ru-old.txt`, the text with the stressed vowels in capitals; an omen
missing there is not recorded in the old speech.

Clips are checked and levelled by voice_in.py's `take`, as every other line is. Omens recorded
already are skipped. Record two or three of a new language first and have them heard.

### `voice_in.py`

Takes voice clips downloaded from Google AI Studio into the game and checks them by ear.

```
python tools/voice_in.py <ru-old|ru|en> <text key> [...]   # -> assets/voice/<lang>/<key>.wav
python tools/voice_in.py --named                          # every tropa__<lang>__<key>.wav waiting
```

A clip the page helper saved under its own name (`tropa__<lang>__<key>.wav`) says which line it
is, so clips from several tabs at once can be taken in whatever order they arrived.

AI Studio saves every clip as `Generated Audio <date>.wav`; the waiting ones are taken in the
order they arrived, one per key. Each clip is then heard by a local Whisper (tools/hear.py) and
scored against the line it should say, taken from assets/i18n/<lang>/*.ftl: a clip that was
saved before the model finished, or that dropped words, shows as a low score and is left in
Downloads under `REJECTED <key>.wav` instead of going into the game. A good clip is levelled on the
way in (see `level`), because the model's voice sinks towards the end of a page; loudness by fifths
of the clip is printed before and after.

### `hear.py`

Listens to voice clips with a small local Whisper and says what was actually spoken.

```
python tools/hear.py <lang> <file.wav> [...]            # print what each clip says
python tools/hear.py <lang> --want "<text>" <file.wav>  # also score it against the text it should say
```

The score is the share of the wanted words that were heard, in order. A text-to-speech model
that drops the end of a long line shows up here as a low score, without anyone listening.
Needs `pip install faster-whisper`; the model is fetched on first use.

### `find_repeats.py`

Finds voice clips in which the line is said more than once ("Угадай, кто я. Угадай, кто я").

```
python -X utf8 tools/find_repeats.py <ru-old|ru|en|all> <bark|story|omen> [who ...]
```

Each clip is heard by the local Whisper (tools/hear.py) and set against its line. A clip is
doubtful when Whisper hears a run of three or more words twice that the line has once, when it
hears half as many words again as the line holds, or when the clip is far longer than the line
takes to say. Doubtful clips are listed in `_inbox/voice/repeats-<kind>.txt` (appended, one run
after another, so a long check can be done in batches: one speaker at a time).

### `redo_doubled.py`

Records again the battle lines found said twice (or with the style read aloud) and hears them again.

```
python -X utf8 tools/redo_doubled.py <lang> <bark-key> [<bark-key> ...]
```

Each bad clip is moved to `_inbox/voice/doubled/<lang>/`, recorded again by tts_api.py and heard by
Whisper; one still said twice goes round again, three times at most.

### `voice_join.py`

Joins a page that had to be read in parts into one clip, then takes it in like voice_in.py.

```
python tools/voice_join.py <ru-old|ru|en> <text key> <parts>
```

Some pages the speech model will not read to the end in one run; they are read in `parts` runs, cut
at sentence ends (the page helper `voicePart` in the browser does the same cut). The newest `parts`
clips in Downloads are joined in the order they arrived, with a short pause between them, and the
whole is scored and levelled against the full text of the page.

### `voice_split.py`

Cuts one narrated scene from Google AI Studio into its pages and takes them into the game.

```
python tools/voice_split.py <ru-old|ru|en> <text key> [...]   # -> assets/voice/<lang>/<key>.wav
```

A scene is read in one run, one speech block per page, so the voice keeps its tone from page to
page. The newest `Generated Audio*.wav` in Downloads is that run. A local Whisper hears it with
word timings, the heard words are lined up against the pages' texts from assets/i18n, and the clip
is cut in the quietest spot of the pause between one page's last word and the next page's first.
Each piece is then scored like tools/voice_in.py does; nothing is written unless every page passes
(the run stays in Downloads to be looked at).

### `barkpage.py`

Writes _inbox/barkpage.js: everything the AI Studio speech page needs to record battle lines alone.

```
python tools/barkpage.py
```

The script carries every line not yet recorded (docs/prompts/voice/barks.<lang>.txt minus the clips
in assets/voice/<lang>/), the casting, and an in-page loop. Load it into the page (a file input and
the browser tool's upload), then:

```
tropaStyle('volkolak', 'ru')            // the style (Scene) text for that speaker in that language
await tropaRecord('volkolak', 6, 'ru')  // the next six missing lines of that speaker in that language
```

One language per run: each language needs its own style, or the accents bleed into each other
(modern Russian read the old way, English without its Slavic accent).

Each finished clip is saved straight from the page as `tropa__<lang>__<key>.wav`, so several tabs
can record at once; take them in with `python tools/voice_in.py --named`. Pick the speaker's voice
(barks.voices.txt) and fill the Style box by hand first: the page ignores a style set by script.

### `barkjs.py`

Prints the page helpers for reading battle lines in AI Studio's speech playground.

```
python tools/barkjs.py <who> [<who> ...]   # e.g. volhv volkolak
```

Paste the output into the page (the browser's JavaScript tool) after "Create new dialog" and
after picking the speaker's voice (docs/prompts/voice/barks.voices.txt). Then for each line:
`barkGo('<lang>', 'bark-<who>-<event>')`, ctrl+Return to run, download, and take the clips in with
`python tools/voice_in.py <lang> <keys>`. Only the lines of the named speakers are embedded.

### `barkftl.py`

Adds to assets/i18n/<lang>/voice.ftl every battle line of docs/prompts/voice/barks.<lang>.txt it lacks.

```
python tools/barkftl.py
```

The speech scripts are the source: for old Russian they carry the stressed vowel in capitals,
which the game's text (and voice_in.py's check) does without, so those capitals are lowered,
leaving only a sentence's first letter and the names of gods and places.

## Sound and music

### `sfx_api.py`

Makes the game's sound effects with the ElevenLabs sound-generation API.

```
python tools/sfx_api.py [name ...] [--takes 3]      # all of SOUNDS when no names are given
python tools/sfx_api.py --pick <name> <take>        # put another take into the game
python tools/sfx_api.py --ambience [act1 ...]       # the looping battle ambience of each act
```

The key is read from `.secrets/elevenlabs_key.txt` (gitignored). Each sound is generated `--takes`
times; every take is cut to its sound (silence trimmed from both ends, with a short fade so the cut
does not click) and kept as `_inbox/sfx/<name>_<take>.wav`, the untouched take as `raw/<name>_<take>.mp3`. The first take goes into the game as
`assets/sfx/<name>-flow.wav`, which the game prefers over its synthesised stand-in.

### `gen_sfx.py`

Synthesised stand-in sound effects, used until a generated `<name>-flow.ogg` sits next to them.

```
python tools/gen_sfx.py [--force]
```

Dry and short, as the sound brief asks: no music, no reverb tail, 48 kHz mono.

### `gen_music.py`

Synthesised stand-in music, used until a generated `<name>-flow.ogg` sits next to it.

```
python tools/gen_music.py [--force] [--only <name-prefix>]
```

Every track is a seamless loop built on the game's five-note leitmotif (A - C - D - E - C)
in a minor mode: plucked gusli, a bowed drone, frame drums, a wordless choir, a bell.
Battle tracks come as three layers of equal length (`_l1` drums and bass, `_l2` melody,
`_l3` choir and heavy drums) that the game stacks and fades; the rest are single loops.
22 kHz mono keeps the files small; the generated tracks that replace them need not be.

### `music_in.py`

Takes a track downloaded from Flow Music into the game.

```
python tools/music_in.py "<file in Downloads>" <track name>
```

Writes assets/music/<track name>-flow.ogg (Vorbis), which the game prefers over the
synthesised stand-in, and moves the download into _inbox/ so it is not picked up twice.

## Text

### `check_strings.py`

Reports content ids that have no name in a language file.

```
python tools/check_strings.py
```

Reads ids out of assets/data/**/*.ron with a regular expression (good enough for
`id: "..."` at the start of a definition) and looks for the matching Fluent key.
Exits with 1 if anything is missing, so it can gate a release.

## Release and analytics

### `pack_assets.py`

Packs the game's assets for shipping, smaller, into target/pack/<profile>/assets.

```
python -X utf8 tools/pack_assets.py desktop   # by eye and ear the same: WebP q92, Ogg Vorbis
python -X utf8 tools/pack_assets.py mobile    # by eye and ear the same: WebP q90, Ogg Vorbis
python -X utf8 tools/pack_assets.py mobile --only cards   # one folder, to look at
```

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

### `google_sheets_telemetry.js`

«Тропа в Навь» — the balance sheet. Google Apps Script bound to a Google Sheet.

 The game posts one JSON record per finished run (crates/game/src/analytics.rs, `RunRecord`,
 schema 2). doPost only appends raw rows; the summary sheets are rebuilt from the raw ones by
 `rebuild()` (menu «Аналитика → Пересчитать», and an hourly trigger made by `installTrigger()`).
 Setup and how to read the sheets: docs/telemetry_and_analytics.md.

 Raw sheets (one row per…):
   Забеги       run             Игроки       player in a run
   Бои          fight           Предложения  card/relic/potion offered (taken or not)
   Карты_игра   card per player per run (plays, damage, block)
 Summary sheets: Карты, Реликвии, Враги, Встречи, Герои, Сборки.

### `make_tools_doc.py`

Writes docs/tools.md: every script in tools/ with what it is for and how it is run, read off
the script's own docstring (or a JavaScript file's first comment).

```
python tools/make_tools_doc.py
```

Run it after adding or changing a tool; the page groups the tools by what they work on. A tool
whose docstring is missing shows up under "Undocumented": write one.

## Undocumented or other

### `downed_in.py`

Takes a maimed hero's picture on the Edge, painted in Flow by tools/flow_downed_cmds.py, into the game.

```
python tools/downed_in.py <flow picture> <hero><mark>
```

The Flow picture is an edit of `_inbox/flow_up/down_<hero>.png`, the hero's downed picture on a
green square; it is scaled back to that square, cut back to the downed picture's own sheet, keyed
(tools/key_flood.py) and saved as `assets/heroes/<hero><mark>_downed-gemini.png`, so it lies exactly
where the whole downed picture lies.

### `flow_downed_cmds.py`

Prints the Browser MCP commands that paint a maimed hero's picture on the Edge (`_downed`) in Flow.

```
python tools/flow_downed_cmds.py <hero> [<mark> ...]
```

A hero down on the Edge kneels in `<hero>_downed`; one who has lost the head or an arm kneels in
`<hero><mark>_downed` (`_noweapon`, `_noshield`, `_noarms`, `_headless`, `_headless_noweapon`, ...,
see `short_of` in crates/game/src/battle.rs). Each is an edit of the hero's whole downed picture:
the commands upload it on a green square (`_inbox/flow_up/down_<hero>.png`, written here), then ask
for one edit per missing mark without waiting, and print nothing else. With no marks, every mark the
hero has a standing picture for and no downed one yet. Flow orders the tiles as they finish, so
fetch them by eye: `python tools/downed_in.py <tile> <hero><mark>` takes one in.

WEAPON / SHIELD say which arm is which in each hero's downed picture: the weapon arm is the one
`_noweapon` loses.

### `foe_sheet.py`

Sheets a foe's every picture to look at: is it all there and does it all fit together?

```
python tools/foe_sheet.py <id> [<id> ...]        # target/foe_<id>.png each
python tools/foe_sheet.py --all <file>           # the ids listed in a file, one per line
```

One row per foe: the resting picture; head over body with the rig's head joint marked (red dot,
read from crates/game/src/battle/rig.rs) and anything the parts miss of the resting picture in
magenta; the head alone; both corpses; every pose (`_strike`, `_hurt`, `_guard`, `_cast` and any
`_<move id>`). On grey, so a black figure shows and a green fringe stands out.

### `gorynych_in.py`

Takes Gorynych into the game as one body with three heads on their own necks.

```
python tools/gorynych_in.py [<dir>]
```

Reads from `<dir>` (default `_inbox/flow_up`) the Flow pictures `gor3.png` (the whole dragon),
`gor3_body.png` (the same dragon, all three necks cut halfway up), `gor3_mask.png` (the same
dragon with the fire head magenta, the venom head cyan, the thunder head yellow), `gor3_dead.png`
and `gor3_deadhl.png` (lying dead, with and without heads). Writes into assets/enemies, all on
the same 1024 sheet so they lie exactly over each other:

- `gorynych-gemini.png` the whole dragon, `gorynych_body-gemini.png` the body with three stumps;
- `gorynych_<fire|poison|thunder>_head-gemini.png`: each head with the part of its neck the body
  lacks (whatever of the whole dragon the body does not hold goes to the nearest mask colour);
- `gorynych_dead-gemini.png`, `gorynych_dead_headless-gemini.png` (1536x1024).

It prints each head's neck joint (where it meets its stump, as fractions of the sheet) for
`crates/game/src/battle/rig.rs`, and writes target/gorynych.png to look at.

### `voice_once.py`

Keeps the first saying of a clip the speech model said twice, or began to say again.

```
python tools/voice_once.py <file.wav> [...]          # cut these at their longest inner pause
python tools/voice_once.py --scan <folder> [<who>]   # list clips that look said twice
python tools/voice_once.py --text <lang> <key> ...   # keep only the line's first saying, found
                                                     # by Whisper (also drops a prompt read aloud)
```

A doubled clip has a long pause in its middle (over 0.6 s) with speech on both sides; the cut is
made just after the first saying, with a short fade. The scan only lists: listen before cutting,
since a slow line ("Тише… тише…") pauses on purpose.
