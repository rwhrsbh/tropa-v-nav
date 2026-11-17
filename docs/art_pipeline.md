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
- Work in parallel: open a second Flow project in another tab (`tab new <project url>`, then
  `tab select <id>` between calls) and send whole batches without waiting (upload once, then one
  prompt after another 3 s apart). While a picture is still being painted its tile already shows
  the attached input (a ~150 KB file): wait until the titles stop changing, or reload, before
  taking tiles. After a reload the grid only holds 512-pixel previews: name the tiles by eye from
  the previews, then open each wanted tile (its edit view) and save the big picture, tagging the
  `img` wider than 300 pixels: `eval` it a `data-big` attribute and `download "img[data-big]"`.
- Flow's own titles say roughly what is in each tile; they come back in finishing order, never in
  the order sent. Always sheet a batch and name every tile by eye.

**Gemini** (gemini.google.com, account u/3) is the fallback. Never the Flash-Lite model (the user
rejects its quality; check the model picker first). Downloads need one real click on the page
first. The limit is a rolling ~5 hour window.

**Gemini API** (`.secrets/gemini_keys.txt`) image quota is 0 on these keys: use Flow.

## Style

The master style tail is in `prompts/00_etalons.md`. Figures are painted on flat pure green
`#00FF00`, no ground, no shadow, no green on the figure, the whole figure inside the frame with a
margin. Heroes face right, foes face left.

Before writing a hero's look into a new prompt, sheet two or three of that hero's existing pictures
and describe what is there (a look written from memory once cost ten repaints). Yemelya: "a plump
clean-shaven grown peasant man with a round face and a blond bowl haircut, in a patched off-white
linen shirt with a red collar and a red sash, grey trousers and bast shoes", always "grown man".
Flow's child-safety filter sometimes refuses a Yemelya gore picture anyway: never reword the prompt
to get past it. Tell the user which tile was refused; they resend the same prompt in Flow and it
passes on the second or third try. Reload, then take the new picture from the edit view.

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

Painting the missing poses of a foe in Flow:

- `python tools/flow_pose_cmds.py <id> [strike hurt guard cast]` prints the Browser MCP block for
  one foe, one pose at a time (about a minute each). With no poses named it paints what the foe
  lacks for its moves: `_hurt` always, `_strike` for attacks, `_guard` for defending, `_cast` for
  buffs, curses, summons and specials. The foe is described by its rig prompt or its line in
  `prompts/01_enemies.md`; the resting picture on a green square (`_inbox/flow_up/rig_<id>.png`)
  is attached as the look only.
- `python tools/flow_pose_cmds.py --batch <id> <id> ...` asks every missing pose of several foes
  one after another without waiting (Flow paints them side by side), waits once and saves every
  tile as `_inbox/flow_up/batch/<n>.png`, with `batch.txt` naming each tile. An uploaded picture
  takes a tile of its own; tiles come newest first. One browser call may run five minutes at most,
  so the batch is also written as `submit.txt` (the asking) and `fetch.txt` (the saving): send
  them as two calls, the second a minute after the first ends.
- Flow orders the tiles as they finish, not as they were asked. The fetch therefore starts by
  printing every tile's own title (its `aria-label`, e.g. "Crone in defensive crouch"); paste that
  line into `_inbox/flow_up/batch/titles.txt` and `pose_batch_in.py` names each tile's pose from
  its title, and its foe from the `rig_<id>.png` upload tile. Check the result by eye anyway.
- The grid holds the full 1024 pictures only while the page has been open since they were made;
  after a reload it shows 512-pixel previews. Then open each tile (the edit view) and save its big
  picture instead: tag the `img` wider than 300 pixels with an attribute and `download` that.
- After an upload the ingredients panel's backdrop can stay over the prompt box; the commands
  press Escape after each upload for that.
- Poses no table fits (a cat on its post, birds) are written per foe in `OWN_POSES`.
- Flow is used from the Google account it was set up in; after a sign-out the project is only at
  `flow.google.com/u/<n>/project/<id>` for that account's number.
- A foe that gains block (any `Block` in its moves) needs a `_guard` too: the game shows the
  guard pose whenever block is gained. Four-legged beasts get their own poses (all paws on the
  ground; the cast is a howl), or the model paints them on two legs or with five.
- `python tools/pose_batch_in.py --look` lays the batch out in `target/batch.png`;
  `python tools/pose_batch_in.py [<n> ...]` takes every pose in, skipping the tile numbers given
  (the bad ones, to be asked again).
- Look at every pose. What goes wrong: a third arm, a weapon with two shafts, a pose that is the
  resting one again. A long list of "no extra limbs" does not help; what helps is never asking for
  a hand to be free that holds something at rest: every pose keeps what the figure holds in the
  same hand (`POSES` in the script says so in each prompt).

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
   in `rig.rs`. The printed head joint sits three quarters down the head's box, which is the chin
   on a short head but lands on the chest under a long beard or long hair (bolotnitsa, karna,
   miller, domovoy): then read the joint off the neck stump in the preview (x, y as fractions of
   the picture) and write that instead. The joint is the pivot the head nods and flies off
   around. Small specks Flow leaves in the background are dropped from the parts and corpses.

`tools/flow_rig_cmds.py <id> "<who>" "<head>" [his|her|its]` prints the whole Browser MCP command
block for steps 1 to 5 (about four minutes in Flow), with exactly the prompts above; it expects the
resting picture on a green square at `_inbox/flow_up/rig_<id>.png` and saves
`_inbox/flow_up/<id>_{mask,body,dead,deadhl}.png`. Keep the old files before step 6 overwrites
them.

`tools/rigparts.py` is the part cutter under `rig_in.py` (`PARTS=head,arm` limits the parts,
`GROW=` widens the seam). Describe wings, claws and paws per creature, never generic "arms".

## Maimed heroes on the Edge

A hero down on the Edge kneels in `<hero>_downed`; one short of the head or an arm kneels in
`<hero><mark>_downed`, `<mark>` one of `_noweapon`, `_noshield`, `_noarms`, `_headless`,
`_headless_noweapon`, `_headless_noshield`, `_headless_noarms` (`short_of` in
`crates/game/src/battle.rs`). Without one the hero stands in the maimed standing picture instead of
kneeling. Every hero form has all of them (`volkolak_beast` too).

- `python tools/flow_downed_cmds.py <hero> [<mark> ...]` prints the Browser MCP commands: it puts
  the downed picture on a green square (`_inbox/flow_up/down_<hero>.png`) and asks one removal edit
  per mark. A headless maimed mark is asked of the maimed downed picture once that exists, beheading
  only: asked of the whole picture at once, Flow took the Bogatyr's shield and left the arm.
- Name each arm by what it holds in the downed picture (`WHO` in the tool): the weapon arm is the
  one `_noweapon` loses. On the Volkolak man and beast the shield arm is the near arm, the weapon
  arm the knife arm / far arm. Where an arm reaches the ground, say so ("the long arm that goes
  down to the clawed hand on the ground"): otherwise Flow cuts the shoulder and leaves the hand.
- Always remove, never add: asking Flow to paint a stump onto a figure that just lacks an arm reads
  as a third arm. Two whole arms plus a stump is three arms: repaint.
- Check every tile against the downed picture: Flow often removes only what the arm holds, or
  nothing at all. Then `python tools/downed_in.py <tile> <hero><mark>` lays it back exactly where
  the downed picture lies.

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
