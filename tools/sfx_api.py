"""Makes the game's sound effects with the ElevenLabs sound-generation API.

    python tools/sfx_api.py [name ...] [--takes 3]      # all of SOUNDS when no names are given
    python tools/sfx_api.py --pick <name> <take>        # put another take into the game
    python tools/sfx_api.py --ambience [act1 ...]       # the looping battle ambience of each act

The key is read from `.secrets/elevenlabs_key.txt` (gitignored). Each sound is generated `--takes`
times; every take is cut to its sound (silence trimmed from both ends, with a short fade so the cut
does not click) and kept as `_inbox/sfx/<name>_<take>.wav`, the untouched take as `raw/<name>_<take>.mp3`. The first take goes into the game as
`assets/sfx/<name>-flow.wav`, which the game prefers over its synthesised stand-in.
"""
import json
import os
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
import wave

import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
KEY = os.path.join(ROOT, ".secrets", "elevenlabs_key.txt")
INBOX = os.path.join(ROOT, "_inbox", "sfx")

# what each cue in audio.rs is heard on, and how long it may last (None: the model decides)
SOUNDS = {
    "block": ("Heavy wooden round shield blocking an axe blow, dull wooden thud with a ringing iron rim, single hit, dry, no music", 1.0),
    "card_draw": ("Parchment card drawn from a deck, soft quick paper swish, very short, dry, no music", 0.5),
    "card_play": ("Playing card slapped onto a wooden table, crisp paper snap, very short, dry, no music", 0.5),
    "click": ("Tiny wooden toggle click, soft dry interface tap, very short, no music", 0.5),
    "death": ("Ghoulish monster dying, rattling last breath fading into silence, dark horror, no music", 2.0),
    "defeat": ("Defeat sting: slow low war drum and a mournful horn, dark Slavic folk horror, 3 seconds", 3.0),
    "downed": ("Warrior collapsing onto forest ground, armour thud and a pained groan, no music", 1.5),
    "gold": ("Old copper coins clinking into a leather pouch, short, no music", 1.0),
    "heal": ("Healing magic: soft warm shimmer with herbal crackle and a calm breath, no music", 1.5),
    "hit": ("Sword slash cutting into a monster, short punchy flesh impact, no music", 0.7),
    "hit_heavy": ("Brutal mace blow crushing bone, deep heavy impact, short, no music", 1.0),
    "potion": ("Cork popped from a small glass bottle, then a quick gulp of liquid, no music", 1.5),
    "raised": ("Revival magic: ghostly rising whoosh and a deep gasping inhale, no music", 2.0),
    "ready": ("Two quick knocks on an oak door, dry, no music", 0.7),
    "round": ("Single deep war drum hit with a long dark reverb tail, no music", 2.0),
    "rune": ("Rune of thunder igniting: crackling electric spark and a low rumble, short, no music", 1.5),
    "status": ("Curse taking hold: eerie hissing whisper with a dark shimmer, short, no music", 1.5),
    "victory": ("Victory sting: bright folk horn fanfare with frame drums, triumphant, 3 seconds", 3.0),
    # the path map, voted on together
    "map_vote": ("Small carved bone token set down firmly on an old parchment map on a wooden table, single dry tap, no music", 0.6),
    "map_nudge": ("Impatient double knock of knuckles on an old wooden table, short, dry, no music", 0.6),
    "lot_tick": ("Single dry wooden click of a spinning fortune wheel peg, very short, no music", 0.3),
    "lot_land": ("Fortune wheel stops with a heavy wooden thunk and a low ominous bell toll, no music", 2.0),
    "card_shuffle": ("Deck of old parchment cards riffle-shuffled by hand, quick fluttering paper riffle then a soft tap square, no music", 1.2),
    "card_gain": ("Parchment card swooping through the air and sliding into a deck, quick whoosh then a soft paper slap, no music", 0.8),
}


# the book of the tale: assets/sfx/book_<name>-flow.wav, takes kept like the others
BOOK = {
    "book_flip": ("Single page of an old thick parchment book turned quickly, crisp paper flutter and soft landing, no music", 0.8),
    "book_rustle": ("Old parchment page being bent and dragged slowly by a hand, soft crackling paper rustle, no music", 1.5),
    "book_tear_small": ("Small piece of old thick paper torn off, short sharp rip, no music", 0.6),
    "book_tear_big": ("A whole page ripped out of an old leather-bound book, long tearing rip with fibres snapping, no music", 1.2),
    "book_peel": ("Two glued paper pages slowly peeled apart, sticky crackling separation, no music", 1.5),
    "book_clasp": ("Brass clasp of an old leather book unlatched, small metallic click, no music", 0.5),
    "book_open": ("Heavy old leather book cover opened, creaking spine and a soft thud on a wooden table, no music", 1.5),
    "book_close": ("Heavy old leather book slammed shut, deep thud and a puff of dust, no music", 1.0),
    "book_scrap": ("Small scrap of paper falling and settling onto a wooden table, very soft, no music", 0.6),
}
SOUNDS.update(BOOK)

# limbs and heads
SOUNDS.update({
    "limb_off": ("A cleaver hacking through an arm, wet crunch of bone and a splatter of blood on the ground, single hit, no speech, no music", 1.2),
    "head_off": ("A head chopped off: wet heavy chop, a long squelching spurt of blood, then the head thudding on the ground, no speech, no music", 2.2),
})
# what comes out of a body that walks with no head, in place of its words
for i, (what, seconds) in enumerate([
    ("Wet gurgling from an open throat, thick bubbling blood, no voice, no words", 1.5),
    ("A slow wet squelching gasp of air through a severed windpipe", 1.5),
    ("Bubbling, sputtering spray of blood from a stump, a short burst", 1.0),
    ("A long liquid rattling gurgle that rises and sinks, horror", 2.0),
    ("Short sticky squelch and a spit of blood", 0.8),
    ("A choking hiss of air through blood, wet and slow", 1.5),
    ("Three wet glugging pulses of blood from a neck", 1.5),
    ("A low wet moan of air with no mouth to shape it, bubbling, no words", 2.0),
], 1):
    SOUNDS[f"gurgle_{i}"] = (what + ", no music, no speech", seconds)
# each status laid on: assets/sfx/status_<kind>-flow.wav, the plain "status" for any without one
SOUNDS.update({
    "status_strength": ("Warrior's short fierce growl and a gauntlet clenching, leather and iron creak, power rising, no music, no speech", 1.0),
    "status_tempstrength": ("Quick sharp battle shout swallowed by a rushing whoosh of blood to the head, short, no music, no words", 0.8),
    "status_drowse": ("Slow heavy drowsy sigh with a soft dreamy hum fading away, sleepy, no music, no words", 1.5),
    "status_fortitude": ("Heavy chainmail settling and leather straps pulled tight, a solid stone grind, short, no music", 1.0),
    "status_vulnerable": ("Brittle armour cracking open, dry splintering crack of a shield, short, no music", 0.8),
    "status_weak": ("Weary limp exhale and a sword point dragging down onto stone, tired, short, no music, no words", 1.0),
    "status_rot": ("Wet squelch of rotting flesh and a short buzz of carrion flies, disgusting, no music", 1.2),
    "status_hex": ("Curse taking hold: eerie hissing whisper with a dark shimmer, short, no music", 1.5),
    "status_bleed": ("Fresh wound opening, a wet tearing slice and blood dripping onto the ground, short, no music, no speech", 1.0),
    "status_fear": ("Sudden ghostly shriek from far away and two fast pounding heartbeats, horror, short, no music", 1.2),
    "status_thorns": ("Thorny brambles snapping and twisting around wood, dry crackling vines, short, no music", 1.0),
    "status_huntmark": ("Short low hunting horn call and an arrow thudding into a wooden post, no music", 1.2),
    "status_valor": ("Brave warrior's shout and a sword hilt banged on a round shield, short, no music, no words", 1.0),
    "status_stun": ("Dazing blow to a helmet: dull clang and a high-pitched ringing in the ears, short, no music", 1.2),
    "status_mist": ("Soft cold breath of drifting fog, a hollow airy whoosh, ghostly, short, no music", 1.2),
    "status_chained": ("Heavy iron chains rattling and a padlock snapping shut, short, no music", 1.0),
    "status_gloom": ("Low ominous drone swelling with a dark cold wind, dread, short, no music", 1.8),
    "card_burn": ("Old parchment card catching fire and crumbling to ash, quick flare and crackle, no music", 1.2),
})
# each foe dies its own way: assets/sfx/death_<foe>-flow.wav, the plain "death" for any without one
SOUNDS.update({
    "death_grey_wolf": ("Big wolf dying, a pained yelp fading into a wet whimper, single sound, no music, no speech", 2.0),
    "death_kikimora": ("Hunched swamp hag dying, a shrill rasping shriek that chokes off, single sound, no music, no speech", 2.0),
    "death_lesavka": ("Small forest girl spirit dying, a thin childlike wail turning into rustling dry leaves, single sound, no music, no speech", 2.0),
    "death_auka": ("Forest echo spirit dying, a hollow far-off cry repeating fainter and fainter, single sound, no music, no speech", 2.0),
    "death_shishiga": ("Wet water imp dying, a gurgling squeal and a splash, single sound, no music, no speech", 2.0),
    "death_mushroom_corpse": ("Rotten corpse overgrown with mushrooms collapsing, soft squelch and a puff of spores, single sound, no music, no speech", 2.0),
    "death_raven": ("Raven killed, one harsh croak cut short and a flutter of falling feathers, single sound, no music, no speech", 2.0),
    "death_likho": ("One-eyed old crone dying, a long bitter croaking groan, single sound, no music, no speech", 2.0),
    "death_shatun": ("Huge bear dying, a deep rumbling roar breaking into a wet last breath, single sound, no music, no speech", 2.0),
    "death_poloz": ("Giant serpent dying, a long hiss fading out with a heavy coil slumping, single sound, no music, no speech", 2.0),
    "death_tree_guard": ("Living tree falling, deep wood cracking and a heavy thud with rustling branches, single sound, no music, no speech", 2.0),
    "death_leshy": ("Ancient forest giant dying, a deep creaking groan of old wood splitting apart, single sound, no music, no speech", 2.0),
    "death_upyr": ("Vampire corpse dying, a choking gurgle of blood and a dry rattling hiss, single sound, no music, no speech", 2.0),
    "death_drowned_maiden": ("Drowned woman dying, a bubbling underwater sigh and water pouring down, single sound, no music, no speech", 2.0),
    "death_igosha": ("Dead infant spirit fading away, a thin eerie whimper, ghostly, single sound, no music, no speech", 2.0),
    "death_anchutka": ("Tiny imp dying, a quick squeaky shriek and a small pop, single sound, no music, no speech", 2.0),
    "death_swamp_light": ("Will-o-the-wisp going out, a soft ghostly hiss and a faint glassy fizz, single sound, no music, no speech", 2.0),
    "death_tinyak": ("Slimy mud creature dying, a thick wet squelching collapse, single sound, no music, no speech", 2.0),
    "death_yrka": ("Night ghoul dying, a hoarse hungry snarl falling into a rattle, single sound, no music, no speech", 2.0),
    "death_vurdalak_elder": ("Old ghoul lord dying, a long guttural moan with a wet crunch of bones, single sound, no music, no speech", 2.0),
    "death_poludnitsa": ("Noon witch dying, a sharp dry woman's scream fading into hot wind, single sound, no music, no speech", 2.0),
    "death_tryasovitsa": ("Fever hag dying, a shuddering teeth-chattering moan dying out, single sound, no music, no speech", 2.0),
    "death_bolotnik": ("Huge swamp lord dying, a deep drowning roar sinking into bubbling mud, single sound, no music, no speech", 2.0),
    "death_nav_soul": ("Lost soul dissolving, a soft ghostly exhale fading into nothing, single sound, no music, no speech", 2.0),
    "death_kostomakha": ("Skeletal hag falling apart, bones clattering to the ground and a dry sigh, single sound, no music, no speech", 2.0),
    "death_mavka": ("Forest nymph dying, a sad high sigh with branches rustling away, single sound, no music, no speech", 2.0),
    "death_koschei_guard": ("Undead armoured guard falling, rusty armour crashing down and a hollow moan, single sound, no music, no speech", 2.0),
    "death_volot": ("Stone giant falling, a low bellow and a heavy rumbling crash, single sound, no music, no speech", 2.0),
    "death_weeper": ("Weeping ghost dying, sobbing that breaks and goes silent, single sound, no music, no speech", 2.0),
    "death_raven_upyr": ("Vampire raven killed, a wet croak and a heavy flapping thud, single sound, no music, no speech", 2.0),
    "death_sirin": ("Bird woman dying, a beautiful sung note breaking into a harsh shriek, single sound, no music, no speech", 2.0),
    "death_black_rider": ("Black horseman falling, a horse's dying snort and armour hitting the ground, single sound, no music, no speech", 2.0),
    "death_volot_giant": ("Colossal giant falling, a mighty groan and a huge earth-shaking crash, single sound, no music, no speech", 2.0),
    "death_ice_crown": ("Crown of ice shattering, sharp cracking and tinkling shards falling, single sound, no music, no speech", 2.0),
    "death_mara": ("Death goddess of winter dying, a long icy woman's wail and ice cracking, single sound, no music, no speech", 2.0),
    "death_bannik": ("Old bathhouse spirit dying, a scalded wheezing cry and a burst of hissing steam, single sound, no music, no speech", 2.0),
    "death_ovinnik": ("Huge black cat demon dying, a furious yowl fading into crackling embers, single sound, no music, no speech", 2.0),
    "death_kladovik": ("Greedy old hoarder spirit dying, a choking wheeze and gold coins spilling on the floor, single sound, no music, no speech", 2.0),
    "death_vodyanoy": ("Fat water spirit dying, a bubbling frog-like groan and a heavy splash, single sound, no music, no speech", 2.0),
    "death_chuchelo": ("Straw scarecrow collapsing, dry straw rustling and old sticks snapping, single sound, no music, no speech", 2.0),
    "death_verlioka": ("One-eyed giant old man dying, a hoarse bellow and a heavy body hitting the ground, single sound, no music, no speech", 2.0),
    "death_viy": ("Ancient earth demon dying, a deep subterranean groan and earth cracking open, single sound, no music, no speech", 2.0),
    "death_koschei": ("Deathless sorcerer king dying, a long dry rasping scream and bones crumbling to dust, single sound, no music, no speech", 2.0),
    "death_death_oak": ("Great cursed oak falling, massive wood splitting and a long crash, single sound, no music, no speech", 2.0),
    "death_death_chest": ("Iron bound chest bursting open, chains snapping and metal clanging, single sound, no music, no speech", 2.0),
    "death_death_hare": ("Hare killed, one short high squeal, single sound, no music, no speech", 2.0),
    "death_death_duck": ("Duck killed, a short strangled quack and a flutter of wings, single sound, no music, no speech", 2.0),
    "death_death_egg": ("Egg crushed, a brittle crack and a wet squish, single sound, no music, no speech", 2.0),
    "death_koschei_shadow": ("Shadow dissolving, a cold whispering hiss fading out, single sound, no music, no speech", 2.0),
    "death_death_oak_chest": ("Cursed oak and iron chest breaking together, wood splitting and chains snapping, single sound, no music, no speech", 2.0),
    # Expansion enemies
    "death_kot_bayun": ("Monstrous giant cat dying, a deep guttural yowl and hissing rattle cut short, single sound, no music, no speech", 2.0),
    "death_mokhovik": ("Ancient moss monster crumbling, deep earth thud and dry moss rustle, single sound, no music, no speech", 2.0),
    "death_nochnitsa": ("Screeching night striga dying, a high piercing scream dissolving into night wind, single sound, no music, no speech", 2.0),
    "death_baba_yaga": ("Evil old witch falling, a cackling croak breaking into coughing rasps, single sound, no music, no speech", 2.0),
    "death_polunochnitsa": ("Midnight phantom dying, an eerie cold wail vanishing into silence, single sound, no music, no speech", 2.0),
    "death_zlyden": ("Tiny misfortune imp dying, a pathetic squeaking whimper and a clatter of stolen coins, single sound, no music, no speech", 2.0),
    "death_morovitsa": ("Death maiden dying, a mournful chilling sigh and rattling scythe clatter, single sound, no music, no speech", 2.0),
    "death_ogneya": ("Fire spirit dying, a loud hiss of extinguishing embers and steam, single sound, no music, no speech", 2.0),
    "death_sea_guard": ("Armored sea guard dying, wet heavy armor clatter and bubbling splash, single sound, no music, no speech", 2.0),
    "death_gagana": ("Iron-beaked monstrous bird dying, a harsh metallic screech and heavy iron wings collapsing, single sound, no music, no speech", 2.0),
    "death_gorynych_fire": ("Dragon head severed, a deafening reptilian roar and erupting flame sputtering out, single sound, no music, no speech", 2.2),
    # Combat mechanics cues
    "moss_absorb": ("Heavy axe blow thudding into thick damp moss, muffled impact with a squelch, dry, no music", 0.8),
    "kinetic_burst": ("Explosive kinetic shockwave erupting from compressed moss and wood, booming shockwave blast, dry, no music", 1.5),
    "spell_mirror": ("Iron bird feathers reflecting magic, sharp crystalline ring and a ricocheting screech, dry, no music", 1.0),
    "mortar_crush": ("Brutal stone mortar slamming down onto ground, crushing heavy stone impact, dry, no music", 1.2),
    "doom_tick": ("Dark hourglass ticking, eerie metallic clockwork chime of death counting down, dry, no music", 1.5),
    "head_regrow": ("Monstrous three-headed dragon flesh regenerating, wet tearing sinews and guttural hiss, dry, no music", 2.0),
    "death_death_egg_solo": ("Egg crushed, a brittle crack and a wet squish, then a faint needle snapping, single sound, no music, no speech", 2.0),
})

# battle ambience under the music, one seamless loop per act: assets/ambience/<name>-flow.wav
AMBIENCE = {
    "act1": "Dark Slavic forest at night: wind in old pines, creaking branches, a distant owl, faint unseen rustling, eerie, no music",
    "act2": "Haunted swamp at dusk: bubbling mud, croaking frogs, buzzing insects, slow dripping water, eerie, no music",
    "act3": "Frozen winter wasteland: howling icy wind, creaking ice, faint far-off wolf howl, desolate, no music",
    "act4": "Underground hall of bones: low cavern drone, distant chains, dripping water, a faint cold breath, ominous, no music",
}


def ffmpeg():
    import imageio_ffmpeg
    return imageio_ffmpeg.get_ffmpeg_exe()


def generate(key, text, seconds, loop=False):
    body = {"text": text, "prompt_influence": 0.4}
    if loop:
        body["loop"] = True
    if seconds:
        body["duration_seconds"] = max(0.5, seconds)
    req = urllib.request.Request(
        "https://api.elevenlabs.io/v1/sound-generation",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "xi-api-key": key, "Accept": "audio/mpeg"},
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def to_samples(mp3_path):
    """Mono 44.1 kHz 16-bit samples of any file ffmpeg can read."""
    raw = subprocess.run([ffmpeg(), "-v", "error", "-i", mp3_path, "-f", "s16le", "-ac", "1", "-ar", "44100", "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.int16)


def cut_silence(a, rate=44100, floor=0.004):
    """The sound alone: quiet below `floor` of its peak (about -48 dB) is cut from both ends, with
    soft edges; the floor is low so reverb tails and soft decays survive."""
    if not len(a):
        return a
    x = np.abs(a.astype(np.float32))
    hop = rate // 200
    n = len(x) // hop
    env = x[: n * hop].reshape(n, hop).max(axis=1)
    loud = np.flatnonzero(env > env.max() * floor)
    if not len(loud):
        return a
    start = max(0, loud[0] * hop - rate // 200)
    end = min(len(a), (loud[-1] + 1) * hop + rate // 20)
    out = a[start:end].astype(np.float32)
    fade_in, fade_out = min(len(out), rate // 500), min(len(out), rate // 50)
    out[:fade_in] *= np.linspace(0, 1, fade_in)
    out[len(out) - fade_out:] *= np.linspace(1, 0, fade_out)
    return out.astype(np.int16)


def save(path, a, rate=44100):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(a.tobytes())


def main():
    args = sys.argv[1:]
    if args[:1] == ["--pick"]:
        shutil.copy(os.path.join(INBOX, f"{args[1]}_{args[2]}.wav"), os.path.join(ROOT, "assets", "sfx", f"{args[1]}-flow.wav"))
        print("picked", args[1], args[2])
        return
    if args[:1] == ["--ambience"]:
        key = open(KEY, encoding="utf-8").read().strip()
        os.makedirs(os.path.join(ROOT, "assets", "ambience"), exist_ok=True)
        os.makedirs(os.path.join(INBOX, "raw"), exist_ok=True)
        for name in args[1:] or list(AMBIENCE):
            src = os.path.join(INBOX, "raw", f"amb_{name}.mp3")
            open(src, "wb").write(generate(key, AMBIENCE[name], 22.0, loop=True))
            # a loop is kept whole: trimming would break the seam
            a = to_samples(src)
            save(os.path.join(ROOT, "assets", "ambience", f"{name}-flow.wav"), a)
            print("ambience", name, f"{len(a) / 44100:.1f}s")
        return
    takes = 3
    if "--takes" in args:
        takes = int(args[args.index("--takes") + 1])
        del args[args.index("--takes"):args.index("--takes") + 2]
    names = args or list(SOUNDS)
    key = open(KEY, encoding="utf-8").read().strip()
    os.makedirs(INBOX, exist_ok=True)
    for name in names:
        text, seconds = SOUNDS[name]
        for t in range(1, takes + 1):
            try:
                mp3 = generate(key, text, seconds)
            except urllib.error.HTTPError as e:
                print(name, t, e.code, e.read().decode(errors="replace")[:200])
                return
            # the untouched take is kept, so it can be cut again differently
            os.makedirs(os.path.join(INBOX, "raw"), exist_ok=True)
            src = os.path.join(INBOX, "raw", f"{name}_{t}.mp3")
            open(src, "wb").write(mp3)
            a = cut_silence(to_samples(src))
            dst = os.path.join(INBOX, f"{name}_{t}.wav")
            save(dst, a)
            print(f"{name} take {t}: {len(a) / 44100:.2f}s")
        shutil.copy(os.path.join(INBOX, f"{name}_1.wav"), os.path.join(ROOT, "assets", "sfx", f"{name}-flow.wav"))


if __name__ == "__main__":
    main()
