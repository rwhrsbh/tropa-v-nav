"""Writes docs/prompts/plots/relics.json: one full image prompt per relic.

    python tools/relic_prompts.py

The subject is the relic's English name unless WHAT says more exactly what to paint.
The file is marked `_raw`: the browser line sends these prompts as they are.
"""
import glob
import json
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
HEAD = "Generate an image, square 1:1 aspect ratio. Game item icon: "
TAIL = (
    " A single object centred and filling most of the frame, on a plain very dark brown background with a soft vignette, "
    "lit from the upper left. Old, worn, handmade. Style: dark painterly digital oil painting, thick visible brushstrokes, "
    "limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk horror. "
    "No text, no letters, no frame, no border, no hands, no people."
)

# where the name alone would not tell a painter what the thing is
WHAT = {
    "father_amulet": "a small round bronze warrior's amulet on a leather cord, stamped with a sun wheel",
    "motanka_doll": "a faceless Slavic motanka rag doll wound with red thread",
    "thunder_staff": "the top of a staff of lightning-split oak, blackened along the crack, hung with bone charms",
    "chur": "a small squat carved wooden boundary idol with a stern bearded face",
    "bratina": "a round-bellied copper drinking bowl for passing around a feast table",
    "plachea": "a small black mourner's kerchief knotted around a lock of grey hair",
    "clan_banner": "a faded red war banner on a spear shaft, embroidered with a branching tree",
    "samobranka": "a folded white linen tablecloth with red embroidery, a loaf and a jug resting on it",
    "sword_kladenets": "an ancient straight broad sword with a plain cross-guard, the blade faintly bright",
    "holey_stone": "a smooth grey river pebble with a natural hole through it, hung on a cord",
    "ladanka": "a tiny leather incense pouch on a neck cord",
    "cricket_cage": "a tiny birch-bark box with air holes, a cricket's feelers showing",
    "witch_ladder": "a knotted cord with nine black feathers tied into it",
    "three_knots": "a short rope tied with three tight knots, one of them half undone",
    "veche_bell": "a large cracked bronze town bell with a rope",
    "living_water": "a small clay flask stoppered with green wax, clear bright water inside, a fresh leaf tied to the neck",
    "dead_water": "a small clay flask stoppered with black wax, dark still water inside, a dry leaf tied to the neck",
    "gusli_samogudy": "a wing-shaped wooden gusli psaltery, its strings blurred as if playing by themselves",
    "alatyr_stone": "a rough lump of warm amber-white stone carved with an eight-pointed star",
    "sivka_bridle": "a horse bridle of worn leather with silver fittings and a few grey horse hairs",
    "raven_of_nav": "a black raven with pale dead eyes perched on a bone",
    "yaga_pestle": "a huge worn wooden pestle scorched at the end",
    "pedlar_scales": "a small brass hand balance with two pans, one holding a coin and one a tooth",
    "folding_icon": "a small three-leaved folding wooden travelling shrine with dark painted panels, no faces visible",
    "likho_eye": "a single large bloodshot eyeball in a small iron cage",
    "leshy_heart": "a knotted heart-shaped burl of living wood with moss and one green shoot",
    "bolotnik_crown": "a crown of rotten roots, reeds and swamp lilies dripping black water",
    "mara_sickle": "a pale bone-handled sickle rimed with frost",
    "black_rider_spur": "a single black iron spur with a long cruel rowel",
    "sirin_feather": "a long iridescent blue-violet feather with an eye-like marking",
    "poloz_scale": "a single golden snake scale the size of a palm",
    "koschei_coin": "a thick old gold coin stamped with a skull wearing a crown",
    "volot_knuckle": "an enormous yellowed giant's knuckle bone the size of a fist",
    "vurdalak_chalice": "a tarnished silver chalice with a dark red stain inside the rim",
    "noon_veil": "a thin sun-bleached white veil with scorched edges",
    "fever_rag": "a filthy yellowed sweat-stained rag knotted around a small bone",
    "ancestral_hearth": "a small clay hearth stove with glowing coals inside",
    "kin_oath": "two forearm-length strips of red cloth knotted together around a knife",
    "common_pot": "a big blackened iron cooking pot with five wooden spoons in it",
    "bog_border_post": "a short carved wooden border post with a notched top",
    "bog_bylina_scroll": "a rolled birch-bark scroll tied with a leather thong",
    "bog_brother_cross": "two small plain wooden crosses tied together by their cords",
    "bog_svyatogor_helm": "a huge ancient pointed iron helmet with a nose guard, far too big for a man",
    "bog_ilya_mace": "a massive six-flanged iron mace with a worn oak haft",
    "bog_outpost_banner": "a tattered grey frontier pennant on a broken lance",
    "vor_three_spinners": "a spindle wound with three threads, one white, one red, one black",
    "vor_last_breath_vial": "a tiny stoppered glass vial with a faint pale mist inside",
    "vol_bone_tally": "a long flat bone cut with rows of tally notches",
    "vol_idol_splinter": "a long splinter of dark carved wood with half of a carved eye on it",
    "vol_thunder_arrow": "a prehistoric flint arrowhead bound to a short shaft with sinew",
    "vol_book_of_doves": "a thick ancient book with wooden covers and iron clasps, closed",
    "wolf_silver_scar": "a tuft of grey wolf fur with a streak of pure white through it, tied with a silver wire",
    "wolf_pack_howl": "a hollow wolf leg bone made into a whistle on a cord",
    "wolf_blood_trail": "a strip of birch bark with a line of dark red drops across it",
    "wolf_hunger_stone": "a smooth black stone with tooth marks gnawed into it",
    "wolf_inside_out_pelt": "a wolf pelt turned inside out, raw hide outward, folded",
    "wolf_knife_in_stump": "a hunting knife driven upright into a mossy tree stump",
    "wolf_full_moon": "a round disc of polished pale bone with a cratered face like the full moon",
    "wolf_first_fang": "a very large yellowed wolf fang set in old silver",
    "zn_weep_garland": "a small garland of purple loosestrife flowers",
    "zn_midwife_knot": "a red woollen cord tied in an elaborate protective knot",
    "zn_fern_in_kerchief": "a mythical fern flower, small and fiery red, lying in an open embroidered kerchief",
    "zn_bereginya_braid": "a long thick braid of fair hair tied with green ribbons and river reeds",
    "zn_bottomless_kettle": "a dented copper kettle, its inside pitch black as if it had no bottom",
    "sk_mara_veil": "a blank white wooden mask half covered by a thin black veil",
    "sk_petrushka": "a Petrushka glove puppet with a long red nose and a pointed cap",
    "sk_showbooth_curtain": "a small patched red stage curtain on a wooden rod",
    "sk_hundred_faces": "a heap of many small carved wooden masks with different expressions",
    "sk_bear_on_chain": "a small carved wooden dancing bear on a brass chain",
    "sk_alt_bear_tooth": "a large brown bear's tooth pierced and hung on a string of small bells",
    "wolf_alt_wolf_berry": "a sprig of glossy red wolfberries with dark leaves",
    "vor_alt_tear_vial": "a tiny glass vial of clear tears with a black thread around the neck",
    "bog_alt_brother_bracelet": "a heavy twisted bronze bracelet, worn smooth",
    "potion_sash": "a wide woven red and ochre cloth sash laid in a coil, five leather loops sewn on it, each holding a small stoppered clay flask",
    "sunflower": "a single wilted sunflower head with dark seeds and a few ragged yellow petals, on a short dry stalk",
    "rattle": "a Slavic wooden ratchet rattle: a fan of thin linden plates strung on a cord with two finger loops",
    "memory_knots": "a length of grey wool cord tied with many small tight knots, three of them stained rust red",
    "holey_pocket": "a torn-off linen coat pocket with a ragged hole in its bottom, one small coin falling through the hole",
    "ash_handful": "a small heap of grey ash on a scrap of birch bark, a few embers still glowing in it",
    "flail": "a kisten flail: a rusty spiked iron ball on a short chain with a worn wooden handle",
    "spindle": "an old wooden hand spindle wound with coarse grey thread, a stone whorl at its end",
    "wooden_gong": "a bilo: a thick dark oak plank hung on two ropes, a wooden mallet leaning against it",
    "icon_lamp": "a small tarnished copper icon lamp hanging on three thin chains, a tiny amber flame behind red glass",
}


def main():
    names = {}
    for f in glob.glob(os.path.join(ROOT, "assets", "i18n", "en", "*.ftl")):
        for line in open(f, encoding="utf-8"):
            m = re.match(r"relic-([a-z0-9_]+) = (.*)", line)
            if m:
                names[m.group(1)] = m.group(2).strip()
    out = {"_raw": True}
    for rid, name in names.items():
        subject = WHAT.get(rid)
        if subject is None:
            plain = re.sub(r"^(The|A|An) ", "", name)
            subject = "an old Slavic peasant's " + plain[0].lower() + plain[1:]
        out[f"relic:{rid}"] = HEAD + subject + "." + TAIL
    unknown = sorted(set(WHAT) - set(names))
    if unknown:
        raise SystemExit(f"no such relics: {unknown}")
    path = os.path.join(ROOT, "docs", "prompts", "plots", "relics.json")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)
    print(len(out) - 1, "relic prompts,", len(WHAT), "described by hand")


main()
