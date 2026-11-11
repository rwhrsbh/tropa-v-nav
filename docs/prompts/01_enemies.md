# Враги — промпты

Файл-цель: `assets/enemies/<id>-gemini.png`. После генерации: `python tools/cutout.py <скачанный файл> enemies/<id> <ширина> <высота>`.

Каждый запрос = **описание** + **хвост**. Размеры: обычный враг 512×768 (портрет 2:3), элита 1024×1024 (1:1), босс 1536×1536 (1:1).

## Хвост для врагов (в конец каждого запроса)

```text
Single full-body creature, three-quarter view facing left, feet on the ground near the bottom edge, whole figure inside the frame with a small margin, nothing cropped. Flat solid pure green #00FF00 background, no ground shadow, no background elements, no green on the character. No particles, sparks, embers, snow, smoke, magic aura or glow effects around the character: effects are added later in the game engine. Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk tale, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, readable silhouette. No text, no letters, no watermark, no frame, no modern objects.
```

Начало каждого запроса: `Generate an image, portrait 2:3 aspect ratio.` (для элит и боссов — `square 1:1 aspect ratio`).

## Акт 1. Чёрный лес

| id | размер | описание |
| --- | --- | --- |
| `grey_wolf` | 512×768 | готово |
| `kikimora` | 512×768 | готово |
| `lesavka` | 512×768 | A lesavka, a small feral forest spirit: a thin child-sized girl with bark-like skin, moss and twigs for hair, huge dark eyes, a stolen leather purse clutched to her chest, glancing back over her shoulder, ready to bolt. |
| `auka` | 512×768 | An auka, a forest echo spirit: a squat pot-bellied old creature with puffed cheeks and a wide open shouting mouth, shaggy grey lichen beard, short bowed legs, hands cupped around its mouth. |
| `shishiga` | 512×768 | A shishiga, a hunched swamp-forest hag: small, knotted and humpbacked, long rotten claws dripping dark slime, matted wet hair, mouldy rags, a crooked yellow-toothed grin. |
| `mushroom_corpse` | 512×768 | A dead peasant overgrown with fungus: a slumped standing corpse in a rotted shirt and bast shoes, bracket fungi and pale mushrooms bursting from the shoulders, neck and one eye socket, a swollen spore-filled belly. |
| `raven` | 512×768 | A tight flock of four large ragged black ravens in flight, beaks open, one with a pale eye, feathers tattered; they overlap as a single dark mass. |
| `likho` | 1024×1024 | Likho the One-Eyed: a tall gaunt giantess in a tattered black sarafan, a single huge staring eye in the middle of her forehead, long bony arms, hair like dry straw, an iron hook in one hand. |
| `shatun` | 1024×1024 | A shatun, a werebear: a huge starving bear standing upright with a human hunter's torn coat stretched over its shoulders and a rope belt, patchy fur, scarred muzzle, human-like hands with claws. |
| `poloz` | 1024×1024 | The Poloz, a giant golden-scaled forest serpent with a crowned head, coiled in heavy loops, mouth half open showing long fangs, old dull gold scales with dark stains. |
| `tree_guard` | 512×768 | A walking dead spruce: a narrow twisted trunk with two branch arms ending in root claws, a knot-hole face, strips of bark hanging loose, a few straw charms tied to its branches. |
| `leshy` | 1536×1536 | The Leshy, lord of the forest: a towering figure of twisted wood and moss, antler-like branches for a crown, a long beard of grey lichen, one shoulder higher than the other, burning pale green eyes, bast shoes on the wrong feet, a huge club made of an uprooted tree. |

## Акт 2. Мёртвая деревня и болота

| id | размер | описание |
| --- | --- | --- |
| `upyr` | 512×768 | An upyr, a Slavic vampire-corpse: a bloated grey villager in burial clothes with a torn shroud, dark-stained mouth and chin, long yellow nails, earth clinging to the hair. |
| `drowned_maiden` | 512×768 | A drowned maiden: a pale young woman in a soaked white burial shift, long wet hair plastered to her face, blue-grey skin, waterweed around her wrists, arms reaching out, water dripping from her fingers. |
| `igosha` | 512×768 | An igosha, the spirit of an unbaptised child: a tiny armless and legless swaddled figure with an oversized crying head, hiding low to the ground, dark tear tracks on its cheeks. |
| `anchutka` | 512×768 | Three anchutkas, small Slavic imps, bunched together: bald, potato-shaped, with stubby wings, goose feet and no heels, pinching fingers and mean grins. |
| `swamp_light` | 512×768 | A will-o'-the-wisp held in a cage of bones and reeds: a pale sickly flame with a faint skull-like face inside, hovering low, strands of marsh grass trailing beneath it. |
| `tinyak` | 512×768 | A tinyak, a creature of pond scum: a thick shapeless humanoid made of mud and duckweed, a wide frog mouth, small eyes, heavy dripping arms. |
| `yrka` | 512×768 | An yrka, a night field spirit: a thin naked grey man wrapped in a dark cloak, glowing cat-like eyes, a hungry gaunt face, long fingers reaching forward. |
| `vurdalak_elder` | 1024×1024 | The Vurdalak Elder, a dead village headman: a broad corpse in a rotted fur coat with a chain of office, a long grey beard stiff with dark stains, a ring of keys, one hand raised in command. |
| `poludnitsa` | 1024×1024 | Poludnitsa, the Noon Lady: a tall woman in a blinding white dress and a wheat wreath, a sun-bleached expressionless face, a long harvest sickle held low, dry stalks clinging to her hem. |
| `tryasovitsa` | 1024×1024 | Tryasovitsa, the Fever sister: an emaciated barefoot woman with loose hair, a sweat-soaked grey shift, shaking hands, sunken burning eyes, a red cloth tied around her brow. |
| `bolotnik` | 1536×1536 | The Bolotnik, master of the bog: an immense fat old man rising waist-deep from black mire, skin like wet clay covered in leeches and waterweed, a wide toad mouth, fish eyes, huge webbed hands. |

## Акт 3. Навь

| id | размер | описание |
| --- | --- | --- |
| `nav_soul` | 512×768 | A soul of Nav: a tall translucent blue-white figure of a dead peasant, edges fraying into mist, hollow eyes, hands folded as in burial, barely touching the ground. |
| `kostomakha` | 512×768 | A kostomakha: a skeleton warrior held together with rawhide and rusted wire, a cracked skull, scraps of mail, a long bone club. |
| `mavka` | 512×768 | A mavka seen from a three-quarter back view: a beautiful pale forest maiden in a thin green dress who has no back, so the open dress shows only a dark hollow; she looks over her shoulder. |
| `koschei_guard` | 512×768 | A guard of Koschei: a tall dead warrior in blackened plate and mail, a closed helm with a narrow slit, a heavy kite shield and a pick-hammer, frost on the armour. |
| `volot` | 512×768 | A volot, an ancient giant of Nav: a slow hulking grey man with stone-like skin, a tiny head, enormous hands, wrapped in old hides, lifting a boulder. |
| `weeper` | 512×768 | A weeper, a professional mourner of the dead: a woman in black with a long white headcloth covering her face down to the mouth, hands raised and wringing, mouth open in a lament. |
| `raven_upyr` | 512×768 | An upyr-raven: a corpse-thin man with great ragged raven wings instead of arms, a beak-like nose, burial rags, talons for feet, swooping forward. |
| `sirin` | 1024×1024 | The Sirin: a great dark bird with the head and chest of a beautiful sad woman wearing a small crown, owl-like wings half spread, long talons, mouth open in song. |
| `black_rider` | 1024×1024 | The Black Rider, Koschei's commander: a rider in black lamellar armour and a horsehair-plumed helm on a gaunt black horse, a long lance and a curved sabre, a black banner on his back. |
| `volot_giant` | 1024×1024 | A colossal volot: a mountain-shouldered giant with skin like weathered granite, moss in the cracks, a beard of roots, both fists raised to strike the ground. |
| `mara` | 1536×1536 | Mara, the lady of winter and death: a very tall pale woman in a long white dress, a crown of ice shards, long black hair, an ice-bladed scythe, bare feet, a calm merciless face. |

## Финал. Остров Буян

| id | размер | описание |
| --- | --- | --- |
| `koschei` | 1536×1536 | Koschei the Deathless on his throne: a skeletally thin ancient king, skin stretched over bone, a crown grown into the skull, eyes like two rotten glowing embers, black fur robes, a throne of fused ribs, a bone sceptre in one hand. |
| `death_oak` | 1024×1024 | A huge dead oak on a bare rock, black and leafless, with an iron-bound chest hanging from its branches on heavy chains. |
| `death_chest` | 512×768 | A heavy old chest of dark oak bound with thick iron bands and a great lock, hanging on chains. |
| `death_hare` | 512×768 | A lean grey hare in mid-leap, ears flat, wild white-rimmed eyes. |
| `death_duck` | 512×768 | A wild duck in flight, wings fully spread, neck stretched forward. |
| `death_egg` | 512×768 | A large pale egg with a faint crack, a thin dark needle visible inside it against the light. |
| `sea_guard` | 512×768 | A sea guard of Buyan: an ancient drowned warrior clad in verdigris-covered bronze plate and barnacle-encrusted mail, closed helm, wielding an ornate blackened iron trident. |
| `ogneya` | 512×768 | Ogneya, maiden of embers: a slender burning spirit woman whose hair is swirling orange embers, scorched dark skin, wielding a whip of liquid fire, trailing smoke and soot. |
| `gagana` | 1024×1024 | Bird Gagana, the mythical iron bird of Buyan: an enormous predator bird with plumage of polished steel and dark bronze, a vicious curved iron beak that reflects light like a mirror, razor talons of solid copper. |
| `gorynych_fire` | 1536×1536 | Zmey Gorynych, three-headed Slavic dragon: an immense coiled dragon with heavy obsidian and dark green scales, three distinct serpentine heads (flame, venom, lightning), massive bat-like wings, powerful taloned limbs. |

## Новые враги дополнения (Акты 1–3)

| id | размер | описание |
| --- | --- | --- |
| `kot_bayun` | 1024×1024 | Cat Bayun: a monstrous predatory grey tomcat the size of an ox, perched high on a rotting wooden post, eyes glowing dull amber, iron claws extending from thick paws, razor needle teeth in a purring snarl, matted pelt. |
| `mokhovik` | 512×768 | A mokhovik, a stout moss-covered forest dweller: squat and round like an ancient damp boulder, covered in thick spongy peat and dripping green lichen, root-like arms, hollow dark eyes, absorbing kinetic shock with quiet menace. |
| `nochnitsa` | 512×768 | A nochnitsa striga, night spirit of nightmares: a gaunt hovering shadow-figure with long tattered grey burial rags, pale spectral face partially obscured by matted hair, elongated iron shears in bone fingers, glowing faint cold white. |
| `baba_yaga` | 1024×1024 | Baba Yaga in her flying mortar: an ancient hunched crone with an iron hooked nose, wild stringy hair, wielding an enormous bronze pestle with both gnarled hands, riding an ancient soot-blackened mortar bound with rusted bands. |
| `polunochnitsa` | 512×768 | A polunochnitsa, midnight hag of sleep and terror: a tall gaunt phantom in a moth-eaten dark indigo shroud, faceless shadow beneath a hood with two pinprick cold violet lights, spindly claws reaching out to weave nightmares. |
| `zlyden` | 512×768 | A zlyden, misery imp: a tiny naked scrawny grey goblin-like creature with an oversized hungry mouth full of crooked needles, clutched stolen copper coins in webbed paws, ragged ears, shifting nervously. |
| `morovitsa` | 1024×1024 | Morovitsa, maiden of pestilence and death: a spectral tall woman in a decaying black lace shroud, pale flesh with black veins, holding an hourglass dripping dark blood and a notched rusted scythe, bringing inescapable doom. |
