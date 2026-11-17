# Враги — полный пайплайн генерации и промпты

> [!CAUTION]
> **КРИТИЧЕСКИ ВАЖНОЕ ПРАВИЛО: ПРОПОРЦИИ И КАДРИРОВАНИЕ (ASPECT RATIOS & MARGINS)**
> В Google Flow (Nano Banana 2.1) **ОБЯЗАТЕЛЬНО** переключайте соотношение сторон в селекторе настроек (Триггер настроек) И явно прописывайте его в первой строке промпта!
> Если пропорция не выбрана явно, генератор по умолчанию выдает `16:9` (альбомный widescreen). В таком формате вертикальные персонажи зверски обрезаются сверху и снизу (отрубленные головы, обрезанные ноги, перья, срезанные ветки), что делает их полностью непригодными для использования в игре!

## 1. Таблица обязательных пропорций и размеров

| Тип ассета | Пропорция во Flow UI | Промпт во Flow | Итоговый размер в игре | Инструмент обработки |
| :--- | :--- | :--- | :--- | :--- |
| **Обычные враги (стоячие)** | `3:4` (портрет) | `portrait 3:4 aspect ratio` | `512×768` | `tools/cutout.py <file> enemies/<id> 512 768` |
| **Элиты и Боссы (стоячие)** | `1:1` (квадрат) | `square 1:1 aspect ratio` | `1024×1024` или `1536×1536` | `tools/cutout.py <file> enemies/<id> 1024 1024` |
| **Герои (стоячие)** | `3:4` (портрет) | `portrait 3:4 aspect ratio` | `768×1024` | `tools/cutout.py <file> heroes/<id> 768 1024` |
| **Маска сочленений (`_colour_mask`)** | Совпадает с базовым (`1:1` или `3:4`) | `square 1:1` или `portrait 3:4` | Совпадает с оригиналом | `tools/rigparts.py` |
| **Стоячий без головы (`_headless`)** | Совпадает с базовым (`1:1` или `3:4`) | `square 1:1` или `portrait 3:4` | Совпадает с оригиналом | `tools/rigparts.py` |
| **Атака (`_strike`)** | Совпадает с базовым (`1:1` или `3:4`) | `square 1:1` или `portrait 3:4` | Совпадает с оригиналом | `tools/cutout.py` |
| **Урон / получение удара (`_hurt`)** | Совпадает с базовым (`1:1` или `3:4`) | `square 1:1` или `portrait 3:4` | Совпадает с оригиналом | `tools/cutout.py` |
| **Защита / щит (`_guard`)** | Совпадает с базовым (`1:1` или `3:4`) | `square 1:1` или `portrait 3:4` | Совпадает с оригиналом | `tools/cutout.py` |
| **Труп с головой (`_dead`)** | **`16:9` (альбомный)** | **`wide 16:9 aspect ratio`** | `1152×768` (обычный) / `1536×1024` (босс/элита) | `tools/cutout.py` |
| **Труп без головы (`_dead_headless`)**| **`16:9` (альбомный)** | **`wide 16:9 aspect ratio`** | `1152×768` (обычный) / `1536×1024` (босс/элита) | `tools/cutout.py` |
| **Арены / Локации (фон битвы)** | **`16:9` (альбомный)** | `wide 16:9 aspect ratio` | `1920×1080` | `tools/cutout.py` / `stands.ron` |

---

## 2. Строгие правила кадрирования и чистоты фона

1. **Видимый зазор со всех четырех сторон (Margins)**:
   - Вся фигура персонажа обязана целиком помещаться в кадр.
   - Ноги / когти / копыта должны стоять на плоскости земли около нижнего края, но **НЕ** касаться и **НЕ** обрезаться нижней рамкой (`diff[-1, :].sum() == 0`).
   - Голова, шлем, корона, рога, поднятое оружие или крылья должны иметь свободный зазор до верхнего края (`diff[0, :].sum() == 0`).
   - Руки, хвосты, крылья, посохи, косы не должны срезаться боковыми рамками (`diff[:, 0].sum() == 0` и `diff[:, -1].sum() == 0`).
   - **Запрещены любые срезанные элементы фона**: никаких веток деревьев, сучьев, валунов, берегов рек, упирающихся в края кадра! Если персонаж сидит на шесте (как Кот Баюн), шест должен быть частью фигуры и иметь отступы до краев.
2. **Сплошной чистый хромакей `#00FF00`**:
   - Фон должен быть монолитным чисто зеленым `#00FF00` (без градиентов, без дымки, без тени под ногами, без текстуры травы или камня).
   - На самом персонаже не должно быть чистого салатово-зеленого оттенка (чтобы не просвечивал при кеинге).
   - Даже для поверженных трупов (`_dead` и `_dead_headless`) фон **ОБЯЗАН** быть чистым зеленым `#00FF00` без текстуры пола или грязи.

---

## 3. Маски расчленения и рига (Rigging Masks & Headless)

В игре работает физический шейдерный 2D-риг с отлетающими головами и вращающимися сочленениями (`crates/game/src/battle/rig.rs`).

### А. Цветовая маска (`<id>_colour_mask.jpg`)
В Flow загружается базовый спрайт персонажа как референс (`.jpg`). Обязательно дожидаться завершения загрузки (`aria-busy="false"` на чипе).
Промпт:
```text
Edit the attached picture: one single image, [square 1:1 / portrait 3:4], no text, no labels, no divider. Keep the SAME character figure, pose, framing, size, position, lighting and plain green background pixel for pixel. Only paint these parts as flat solid colours, clean silhouettes with no shading:
- whole head with hair, beard, crown and face in pure cyan #00FFFF;
- main striking forearm, hand and whole weapon (sabre/scythe/mace/claws) in pure magenta #FF00FF;
- secondary off-arm, hand, shield, second wing or accessory in pure yellow #FFFF00.
Everything else unchanged.
```

### Б. Обезглавленное стоячее тело (`<id>_headless.jpg`)
В Flow загружается базовый стоячий спрайт персонажа.
Промпт:
```text
DECAPITATE the attached figure. Keep the picture exactly the same, same body, pose, framing, size, colours and plain green background, one single image, [square 1:1 / portrait 3:4], no text. Only the whole head is gone: cut off and absent. Where the head was, a raw bloody neck stump with exposed gore and dripping blood, in the same painted style. Nothing else changes.
```

### В. Извлечение частей и регистрация суставов
Скрипт `tools/rigparts.py` сопоставляет маску и тело, вырезает части и вычисляет координаты суставов вращения:
```bash
python tools/rigparts.py assets/enemies/<id>-gemini.png _generated_review/<id>_headless.jpg _generated_review/<id>_colour_mask.jpg assets/enemies <id> _generated_review/<id>_parts_preview.png
```
Скрипт выводит строку для `crates/game/src/battle/rig.rs:FIGURES`:
```rust
("enemies/<id>", &[(Part::Head, Vec2::new(x, y), 1.0), (Part::Shield, Vec2::new(x, y), 0.8), (Part::Arm, Vec2::new(x, y), 1.0)]),
```
Эта строка **обязательно** вносится в массив `FIGURES` в `crates/game/src/battle/rig.rs`.

---

## 4. Спецификация боевых спрайтов (Combat Action Sprites)

Каждый враг и герой в игре имеет полный набор уникальных спрайтов действий на чистом зеленом фоне `#00FF00`:
1. `<id>-gemini.png`: Базовая нейтральная стойка (в 3/4 лицом влево для врагов, вправо для героев).
2. `<id>_strike-gemini.png`: Агрессивный динамичный выпад вперед, замах оружием или когтями.
3. `<id>_hurt-gemini.png`: Резкая отдача назад от мощного удара, тело запрокинуто назад, раскрытый в крике боли рот.
4. `<id>_guard-gemini.png`: Глухая оборонительная стойка, блок щитом, скрещенным оружием или крыльями.
5. `<id>_dead-gemini.png`: Поверженный труп на полу в горизонтальной композиции (`16:9`), глаза закрыты.
6. `<id>_dead_headless-gemini.png`: Поверженный труп на полу в `16:9`, но с отрубленной головой и кровавым срезом шеи.

Обработка всех боевых спрайтов:
```bash
# Для стоячих (элиты/боссы 1024x1024, обычные 512x768):
python tools/cutout.py _generated_review/<id>_<action>.jpg enemies/<id>_<action> <width> <height>

# Для трупов (боссы 1536x1024, обычные 1152x768):
python tools/cutout.py _generated_review/<id>_dead.jpg enemies/<id>_dead 1536 1024
python tools/cutout.py _generated_review/<id>_dead_headless.jpg enemies/<id>_dead_headless 1536 1024
```

---

## 5. Локации и маски стояния ног (Location Stands)

Фоновые локации битвы (`assets/bg/<id>-gemini.png`) имеют размер `1920×1080` (16:9).
Чтобы персонажи не висели в воздухе, не стояли в воде или на вертикальных стенах, создается маска стояния ног:
1. Задник локации подается во Flow с промптом расставить маркеры точек опоры на реальной плоскости земли / твердого пола.
2. **Герои (слева)**: 4 красные точки `#FF0000` (слоты 0..3 от передовой вглубь).
3. **Враги (справа)**: 4 циановые точки `#00FFFF` (одиночный враг встает в центр, остальные распределяются по рангам).
4. Центроиды точек детектируются и записываются в `assets/data/stands.ron`:
```ron
"bg/<backdrop_id>": (
    heroes: [(x1, y1), (x2, y2), (x3, y3), (x4, y4)],
    foes: [(x1, y1), (x2, y2), (x3, y3), (x4, y4)]
),
```
5. Модуль `crates/game/src/battle/stands.rs` читает эту таблицу при старте боя и ставит базовую линию ног каждого персонажа ровно в координаты опорных точек пола.

---

## 6. Базовый мастер-стиль и описания врагов

```text
Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk tale, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, readable silhouette. No text, no letters, no watermark, no frame, no modern objects.
```

Хвост для промпта:
```text
Single full-body creature, three-quarter view facing left, feet on the ground near the bottom edge, whole figure inside the frame with a small margin, nothing cropped. Flat solid pure green #00FF00 background, no ground shadow, no background elements, no green on the character. No particles, sparks, embers, snow, smoke, magic aura or glow effects around the character: effects are added later in the game engine. Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk tale, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, readable silhouette. No text, no letters, no watermark, no frame, no modern objects.
```

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

## Новая волна (ноябрь 2026)

Промпты 23 врагов, 10 значков особенностей и двух карт-статусов — в `docs/prompts/plots/wave.json`;
сообщения для агента Flow собирает `python tools/wave_prompts.py`. Каждый спрайт — одна картинка на зелёном;
`python tools/foe_rig.py <картинка> <id> <512x768|1024> <шея>` вырезает фон и делает `_head`, `_body`, `_dead`, `_dead_headless`
(шея — доля высоты фигуры, либо `x,y` для зверя на четырёх лапах, либо рамка `л,п,в,н` для головы между крыльями или посохом).

Боссы первого и второго актов (Идолище Поганое, Вихорь, Горе-Злосчастье, Злой Домовой), их значки и карта Горе —
`docs/prompts/plots/bosses2.json`. Соловей, Корочун, Баба-Яга и Водяной царь с тех пор — элиты.

## Kot Bayun off his post (`kot_bayun_ground*`)

While `bayun_perch` is set he is drawn on his post (`kot_bayun`, `_strike`, `_hurt`, `_guard`);
once he leaps down the game swaps in the `_ground` set. Each was painted in Flow from the resting
picture attached as the look only (an edit of the perched picture kept the floating perch pose:
ask for a completely new picture). Keyed with `tools/key_flood.py`, taken in with
`tools/pose_in.py`.

- `kot_bayun_ground`: Use the attached picture ONLY for what the cat looks like (huge black scarred cat, orange eyes, bristling fur, painted style, lighting). Draw a completely new picture, square 1:1, one single image, no text: the cat STANDING on the ground on all four straight legs, full side view, body horizontal, facing left, back arched and fur raised, head low and snarling at the viewer's left, tail raised high in a curve ABOVE his back. All four paws touch one flat ground line at the bottom of the picture; nothing hangs below the paws. No post, no stump, no rock. The whole cat inside the frame with a margin on every side. Flat solid pure green #00FF00 background, no ground texture, no shadow, no green on the cat.
- `kot_bayun_ground_strike`: Use the attached picture for what the cat looks like (the SAME huge black scarred cat, orange eyes, same fur, painted style, lighting and size). New pose, square 1:1, one single image, no text: he LUNGES forward to the left, striking with one front paw raised high, claws fully out, mouth wide open in a hiss, hind legs pushing off the ground, body stretched low. No post, no stump. Whole cat inside the frame with a margin on every side, his hind paws near the bottom. Flat solid pure green #00FF00 background, no ground texture, no shadow, no green on the cat.
- `kot_bayun_ground_hurt`: Use the attached picture for what the cat looks like (the SAME huge black scarred cat, orange eyes, same fur, painted style, lighting and size). New pose, square 1:1, one single image, no text: he has just been struck hard and REELS BACK to the right, away from the blow, head thrown back with eyes squeezed shut and mouth open in a yowl, front paws lifted off the ground, hind legs braced, fur flattened, a little dark blood on his shoulder. No post, no stump. Whole cat inside the frame with a margin on every side, his hind paws near the bottom. Flat solid pure green #00FF00 background, no ground texture, no shadow, no green on the cat.
- `kot_bayun_ground_guard`: Use the attached picture for what the cat looks like (the SAME huge black scarred cat, orange eyes, same fur, painted style, lighting and size). New pose, square 1:1, one single image, no text: he crouches low and defensive on the ground, facing left, body pressed down and coiled, ears flattened back, fur bristling, one front paw raised before his face to ward off a blow, tail wrapped around his flank. No post, no stump. Whole cat inside the frame with a margin on every side, his paws near the bottom. Flat solid pure green #00FF00 background, no ground texture, no shadow, no green on the cat.
