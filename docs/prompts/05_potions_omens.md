# Potion flasks, omen pictures, book_spoil

Three batches for Google Flow (Nano Banana, never Flash-Lite), six images a message, each checked
by eye before it is taken in.

## Potions — `assets/potions/<id>-gemini.png`

Shown round, 68 px on the battle belt and 116 px at the pedlar (`battle.rs` `sync_potions`,
`run_ui.rs` `ware`). Same look as the status and intent icons: one painted object on flat pure
black, taken in with `python tools/intentart.py --into potions <id> ...` (black keyed to alpha).

> Paint six separate square icons for a dark Slavic folk-horror card game. Each is one old
> vessel — clay pot, glass vial, birch-bark box, horn or wooden flask — painted and slightly lit
> from within, centred on a flat pure black background, bold silhouette readable when tiny. No
> text, no letters, no frame, no scenery. Each image is a separate picture:

| id | what it shows |
|---|---|
| oak_bark | a bundle of thick oak bark tied with bast, a faint golden glow |
| wolfsbane | a gnarled dark-purple root in a little clay cup, wolf fur caught on it |
| henbane | a pale glass vial with grey-violet henbane flowers pressed inside |
| rot_jar | a cracked clay pot oozing green mould, flies over the lid |
| evil_eye_bead | a blue-and-white glass eye bead on a red thread |
| thunder_flask | a dark iron-bound flask crackling with a white lightning spark inside |
| burning_pitch | a tarred clay pot with black pitch burning at the mouth |
| kvass | a brown clay jug of kvass with froth on the lip and a crust of rye bread |
| strong_kvass | a heavier jug of dark kvass with a glowing amber froth and a red cork |
| bear_fat | a wooden box of white fat with a bear claw laid across it |
| holy_water | a small clear glass vial with a tiny silver cross, cold light inside |
| honey_cake | a round gingerbread cake stamped with a bird, honey dripping |
| party_broth | a sooty iron pot of steaming broth with a wooden ladle |
| shield_resin | a lump of amber pine resin pressed onto a round shield's iron boss |
| battle_mead | a carved drinking horn of red mead with gold froth |
| fear_smoke | a clay censer with thick black smoke curling into a screaming face |
| mist_vial | a pale glass vial filled with swirling grey-blue fog |
| bleeding_thorn | a long black thorn with a drop of blood at its tip, in a red ribbon |
| wake_root | a twisted root with a bright green new shoot, sparks around it |
| last_drop | a tiny crystal vial with a single glowing red drop |
| nettle_brew | a clay mug of dark green nettle brew, nettle leaves over the rim |
| mead | a round wooden ladle-cup of golden honey mead |
| tar | a birch-bark box of thick black tar with a brush in it |
| fly_bottle | a green glass bottle buzzing with black flies inside |
| sleeping_draught | a dark blue vial with a red poppy tied to its neck |
| blood_and_milk | a small jug, half milk-white, half blood-red, swirling together |
| rusalka_tear | a pearl-like teardrop of river water in a clam shell, pale green light |
| fire_water | a clear bottle of vodka with a blue flame dancing on top |
| hoarfrost | a glass vial furred with white hoarfrost, icy blue inside |
| rowan_tincture | a small bottle of red rowan tincture with a sprig of rowan berries |

## Omen pictures — `assets/omens/<id>-gemini.png`

Square (1:1), framed at 780 px beside the omen's words (`run_ui.rs` `omen_picture`). Painted in
the style of the battle backdrops (`assets/bg/act1_battle-gemini.png`): dark painterly Slavic
folk horror, muted greens and browns, mist, one warm or sickly light source, no people's faces
in close-up, no text.

> Paint six separate square illustrations for a dark Slavic folk-horror card game, in the same
> painterly style as before: muted swamp greens, browns and greys, mist, one source of warm or
> sickly light, deep shadows, uneasy and quiet. No text, no letters, no frame. Each image is a
> separate picture:

| id | what it shows |
|---|---|
| well | a blackened log well with a wet chain going down into darkness, a dry bucket |
| crossroads_stone | a moss-scraped boulder at a three-way fork with three carved notches |
| hungry_village | a village of log huts with lit stoves and no smoke; thin children by a fence |
| bannik | the inside of a dark steaming bathhouse, something hunched on the top bench |
| drowned_mill | a still mill wheel over black water, small wet footprints going to the edge |
| hanged_man | a man in bast shoes hanging from a great oak bough at dusk |
| hen_legged_hut | a log hut on huge chicken legs, back to the forest, door ajar |
| rusalka_pool | a warm misty forest pool, a white shirt drying on a branch above it |
| wedding_of_the_dead | long tables in an open field at night, pale guests sitting very straight |
| kurgan | a grassy burial mound at dusk, a fresh pit and a spade at its foot |
| ferryman | a hooded ferryman rowing a dark boat across a misty black river |
| talking_cat | a big black cat on a golden chain around an oak stump |
| crying_child | a swaddled bundle under a spruce in a windfall of broken trees |
| smithy_ruin | a ruined cold forge with glowing bellows and a strange horseshoe on the anvil |
| leshy_trail | a pine with an axe notch, the forest path looping back on itself in fog |
| bog_lights | pairs of pale will-o'-wisps over a log causeway through a bog at night |
| old_shrine | a ring of carved wooden idols in a clearing, one turned outward, mouth smeared |
| frozen_host | ranks of frozen warriors standing on lake ice with spears, frost in their beards |
| pedlar_ghost | a faceless pedlar with a wooden box, his wares laid out on snow |
| bone_mill | a mill grinding white bone flour with no wind and no water, warm light inside |
| krada | an empty funeral pyre burning steadily in a dark clearing |
| blood_oak | a vast old oak with dark sticky bark at its roots and a straw doll hanging |
| forked_path | a forest trail split in two: drag marks to the left, raspberry bushes to the right |
| wolf_shepherd | an old man in a grey sheepskin on a stump, wolves lying meek around him |
| mushroom_ring | a clean ring of red toadstools in a clearing, the grass inside flattened |
| bee_tree | an old hollow bee tree with black honey running down the trunk |
| lost_hunter | a gaunt hunter with a very long beard by a dying campfire |
| owl_omen | an owl perched on a bough in pale daylight, staring |
| hollow_log | a huge rotten fallen trunk with a glint deep inside its dark mouth |
| two_bridges | a rotten plank bridge over a gully, a ford downstream, a bundle under the bridge |
| charcoal_burner | a man seen from behind raking a smoking charcoal pit, sparks rising |
| empty_cradle | an empty cradle hanging from the beam of a roofless hut, snow falling in |
| plague_cart | a cart with empty shafts across a village street, goods under sacking |
| church_and_tavern | a small wooden chapel with one candle facing a tavern bright with many |
| gallows_crows | an empty gallows with rows of crows on it like a council |
| drowned_bell | a belfry sunk in a bog up to its cross, ripples on the dark water |
| widow_bread | a warm loaf on a bare table in a poor hut, a woman kneading in the dark |
| swamp_fork | a log causeway through tall reeds, something spilled along it |
| black_rooster | a woman on a porch at dusk holding a black rooster |
| cellar_door | a cellar hatch in a plank floor, braced from below, a candle beside it |
| mirror_house | a dark room full of uncovered mirrors, a faint second figure in one |
| river_smorodina | a river of fire running through a dark land, no ferry |
| kalinov_bridge | a red-hot glowing bridge over the fiery river, bones beneath |
| ancestors_table | a long table laid for many, silent seated shapes, empty places between |
| nameless_grave | a smooth blank grave stone on breathing earth |
| shadow_market | a silent night market of faceless shadowy traders under lanterns |
| sleeping_host | rows of sleeping armoured warriors shield to shield in a cave |
| two_fires | two campfires either side of a trail, one white flame, one black |
| gamayun | the bird Gamayun with a woman's face on a tree, people sitting around below |
| last_candle | a candle stub on a stone at the edge of total darkness |
| alatyr_stone | the sacred white-burning Alatyr stone on a stormy sea reef, glowing with ancient runes through salt foam |
| waters_of_buyan | two mystical bubbling springs under a cliff, one clear with white mist, one dark as obsidian |
| buyan_oak | an enormous ancient dead oak on a high crag with iron-bound chained chests hanging in cold fog |

## Card `book_spoil` — `assets/cards/book_spoil-gemini.png`

Same as the other card art (`docs/prompts/03_cards.md`): a page torn from an old book, its letters
running like black ink into the reader's hand, rot and mould creeping over the margin.
