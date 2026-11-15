# Pictures: how they are made and taken into the game

Prompts by kind live in [`prompts/`](prompts/): `00_etalons.md` (style, heroes, backdrops),
`01_enemies.md` (foes, sizes, poses), `03_cards.md`, `04_status_icons.md`,
`05_potions_omens.md`, `06_locations.md` (backdrops and stands). This page is the method.

## Where pictures are painted

**Google Flow** (https://flow.google.com, project `2380bc42-…`, Nano Banana 2.1) is the main tool.
Generations there cost no credits (the settings panel shows the cost: 0).

- Drive it through the **Browser MCP** extension (`mcp__browsermcp__browser`; server added with
  `claude mcp add browsermcp -s user -- cmd /c npx -y @browsermcp/mcp@latest`, which needs
  `MSYS_NO_PATHCONV=1` from Git Bash and a restart of Claude Code). Claude-in-Chrome works too
  when it connects.
- Settings (button «Триггер настроек»): aspect 1:1 / 3:4 / 16:9 and the number of pictures x1–x4.
  Always pick the aspect *and* write it in the prompt: the default 16:9 crops tall figures.
- Attach a picture: «Добавить ингредиенты» → «Загрузить» (`upload … <file>`) → click its name in
  the list (or «Добавить в запрос»). A generated tile is attached by right-click → «Добавить в
  запрос».
- Prompt box: `getByRole('textbox', { name: 'Что вы хотите создать?' })`, send «Начать генерацию».
  One picture takes 35–45 s.
- Take a picture out with `download "[role=group] img >> nth=0" --path <file>` (nth=0 is the
  newest tile; the image is 1024 px). No menu clicks needed. The old way (right-click → «Скачать» →
  «1K» into Downloads, then `tools/flowart.py`) also works.

**Gemini** (gemini.google.com, account u/3) is the fallback. Never the Flash-Lite model (the user
rejects its quality; check the model picker first). Downloads need one real click on the page
first. The limit is a rolling ~5 hour window.

**Gemini API** (`.secrets/gemini_keys.txt`) image quota is 0 on these keys: use Flow.

## Style

The master style tail is in `prompts/00_etalons.md`. Figures are painted on flat pure green
`#00FF00`, no ground, no shadow, no green on the figure, the whole figure inside the frame with a
margin. Heroes face right, foes face left.

## Keying and sizes

- `tools/key_flood.py` — the key to use: floods the background in from the picture's edges and
  takes pure-green pockets too, so fur and cloth never get holes (a plain colour key makes holes in
  dark fur). Despills the rim.
- `tools/cutout.py` — the older colour key and `fit` (figure fitted to a sheet, feet at 0.957).
- Sizes: common foe 512×768, elite and boss 1024×1024, hero 768×1024, corpse sheets 3:2
  (1152×768 / 1536×1024), card art 1000×750, relic 256×256, backdrop 1920×1080.
- Foe stature (how tall it stands against a man) is `crates/game/src/battle/stature.rs`.

## Foe poses

`<id>_strike`, `_hurt`, `_guard`, `_cast` are drawn in turn during a fight. A pose may be wider
than the resting picture: the game draws it at its own width (`rig::fit`), so it must keep the
resting picture's scale, never be squeezed into its frame (25 poses were once squeezed to half
height). `tools/pose_in.py <flow pic> <rest stem> <out stem>` takes a pose in at the resting
picture's scale and foot line. Kot Bayun has a second set `kot_bayun_ground*` used while he is off
his post (`bayun_perch` flag gone).

## Rig parts and corpses (the dismemberment)

Each foe is cut into a body and a head (some also arm/shield) for the rig in
`crates/game/src/battle/rig.rs` (`FIGURES`: joint and gain per part), and has two corpses,
`<id>_dead` and `<id>_dead_headless`. Never make these by rotating or cropping the standing
picture (the new wave's first cut did so: square heads, standing figures turned 82° for corpses);
paint them:

1. Upload the resting picture padded to a green square.
2. **Colour mask**: "Edit the attached picture… keep the SAME <who>… pixel for pixel. Only paint
   <the whole head and hair> as ONE flat solid pure cyan #00FFFF silhouette." (Magenta = weapon
   arm, yellow = shield/off arm, when those are cut too.)
3. **Body**: "DECAPITATE the attached figure. Keep the picture exactly the same… Only <the head> is
   gone… a raw bloody neck stump… plain green where the head was."
4. **Dead**: "Use the attached picture for what <who> looks like… lies dead flat on the ground on
   its back, seen from the side, head to the LEFT… along the bottom third."
5. **Dead headless**: attach the dead tile, "DECAPITATE the attached lying body. Keep the picture
   exactly the same…"
6. `python -X utf8 tools/rig_in.py <id> <mask> <body> <dead> <dead_headless>` writes head, body
   and both corpses and prints the `FIGURES` line; check `target/rig_<id>.png`, then put the line
   in `rig.rs`.

`tools/rigparts.py` is the part cutter under `rig_in.py` (`PARTS=head,arm` limits the parts,
`GROW=` widens the seam). Describe wings, claws and paws per creature, never generic "arms".

## Other kinds

- Cards and relics: `tools/flowart.py` (`--relic`, `--story`), prompts in `prompts/03_cards.md`.
- Intent emblems over foes: `assets/intents/<Kind>` and per-move `m_<move id>`, made with
  `tools/intentart.py` (black background keyed by brightness; `--into status|map`, `--ink`).
- Status chips: `assets/status/<key>`, prompts in `prompts/04_status_icons.md`.
- Omen pictures: `tools/omenart.py`. Map tools: `assets/map/tool_*`, hero tokens
  `assets/map/hero_<id>`.
- Backdrops: never edited. A foe's lair is `bg/at_<id>` (or `bg/at_<prefix>` for a part of a
  greater beast, as Gorynych's heads). Places for figures: `assets/data/stands.ron` (a backdrop
  with three foes gets exactly three places, so the middle-out order spreads them).
