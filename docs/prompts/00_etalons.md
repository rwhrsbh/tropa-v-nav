# Эталоны — генерируются первыми

Порядок: мастер-стиль → листы героев → фон на акт → иллюстрация карты каждой редкости.
Всё остальное генерируется с эталоном как референсом (дизайн-док §14).

Правила:
- 4 варианта на ассет, берётся лучший по чек-листу §14; «почти хорошо» не принимается.
- Персонажи — на сплошном хромакей-фоне `#00FF00`, фон вырезается скриптом `tools/cutout.py`.
- Готовый файл кладётся рядом с процедурным под именем `<имя>-gemini.png` — игра берёт его первым.
- **ОБЯЗАТЕЛЬНОЕ УКАЗАНИЕ ПРОПОРЦИЙ ВО FLOW**: переключать селектор настроек и писать пропорцию в промпте (`portrait 3:4` для героев и обычных врагов, `square 1:1` для боссов и элит, `wide 16:9` для трупов и локаций). Дефолтные 16:9 калечат вертикальные фигуры!
- **ОТСТУПЫ (MARGINS)**: вся фигура обязана помещаться в кадр целиком со свободным зазором по всем 4 краям. Никаких обрезанных ветвей, лап, шестов или волос!
- Полный пайплайн генерации масок рига (`_colour_mask`), обезглавливания (`_headless`, `_dead_headless`), боевых поз (`_strike`, `_hurt`, `_guard`, `_dead`) и разметки локаций (`stands.ron`) подробно описан в `docs/prompts/01_enemies.md` и `docs/prompts/06_locations.md`.
- Если генератор отказывает из-за жестокости — заменить слова: blood → dark red paint / ketchup-like
  stains, wound → torn cloth, corpse → slumped figure, gore → dark stains. После двух отказов —
  `status: blocked` в `docs/manifest.json`, остаётся процедурный арт.

## Мастер-стиль (в конец каждого запроса)

```text
Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of soot black #14110F, bone #D9CFB8, rust blood #7A1E14, swamp green #3E4A2E, candle amber #D89A3C, nav blue #4A6A8C. Slavic folk horror, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, realistic anatomy, readable silhouette. No text, no letters, no watermark, no frame, no modern objects, no swastika-like symbols.
```

Добавка для персонажей (после описания, перед мастер-стилем):

```text
Single full-body character, three-quarter view facing right, neutral standing pose, feet on the ground near the bottom edge, whole figure inside the frame with a small margin, nothing cropped. Flat solid pure green #00FF00 background, no ground shadow, no background elements, no green on the character. Portrait 3:4.
```

Для врагов — то же, но `facing left`, и `2:3` для обычных, `1:1` для элит и боссов.

## Герои — `assets/heroes/<id>-gemini.png`, 768×1024 после вырезания

### bogatyr
```text
A huge grey-haired Slavic warrior (bogatyr), face hidden by a riveted iron mask with two empty black eye holes, long grey beard spilling from under the mask, battered rusty chainmail over a dark padded tunic, heavy fur-trimmed cloak. Left arm carries a tall tower shield of black oak with a stern ancestral face carved into it and a copper rim. Right hand holds an iron flanged mace (shestopyor). Forearms wrapped in dirty bandages with old dark red stains, one finger missing on the left hand. Massive, weary, immovable.
```

### vorozheya
```text
A gaunt Slavic witch (vorozheya) in a long linen shift soaked with swamp water and clinging mud, wet black hair braided with small bird bones, eyes covered with milky white cataracts. A rope belt hung with cloth ritual dolls (motanka dolls without faces), one doll oozing black tar. Fingers stained with soot and dark red. One hand raised, pinching a thread, the other holding a doll. Thin, barefoot, unsettling calm.
```

### volhv
```text
A tall old Slavic pagan priest (volkhv) in a heavy wolf-pelt cloak and a birch-bark mask with narrow slits, long beard braided with small wooden charms. Bare forearms carved with freshly cut glowing runes (invented angular marks, pale blue-white light). He holds a tall staff made from an oak split by lightning, its top charred. Embers and snowflakes drift around him. Stern, upright, priestly.
```

### volkolak
```text
A lean wiry Slavic hunter cursed as a werewolf (volkolak), human form: a grey wolf pelt across his shoulders with the wolf's head as a hood, a silver chain around his neck biting into the skin, leather and rough wool clothing, long hunting knife in the right hand. Scarred face, hungry yellowish human eyes, tense crouched readiness.
```

### volkolak_beast — 1024×1024, `assets/heroes/volkolak_beast-gemini.png`
```text
The same hunter transformed into a beast: a hunched towering werewolf with a stretched snarling muzzle, torn skin and patchy grey fur, ribs showing, long clawed arms hanging low, the same silver chain still cutting into the neck, tatters of the hunter's clothes. The eyes remain human. Square 1:1.
```

### znaharka
```text
A stooped middle-aged Slavic herbalist healer (znakharka) in a dark headscarf, face covered with old burn scars, heavy dark dress and apron. Bundles of dried herbs tied to her back, a sickle and two clay jugs at her belt, a small birch-bark pot hanging from one hand. Arms stained to the elbows with green plant juice and dark red. Tired, kind, hard.
```

### skomoroh
```text
A skinny Slavic jester (skomorokh) in a patchwork kaftan sewn with many small brass bells, wearing a plain white wooden mask; where the mask ends the skin is stitched together from mismatched pieces. A grin cut ear to ear and sewn shut is visible below the mask. Three wooden masks hang at his belt: a bear, a goat, and a blank white mask. He holds a tambourine. Loose, mocking bow-legged stance.
```

## Фоны — `assets/bg/<id>-gemini.png`, 3840×2160 (16:9)

Общая добавка:
```text
Wide 16:9 game battle background, empty stage: clear flat ground band across the lower third where characters will stand, no characters, no animals, nothing in the foreground centre. Depth in layers: far, middle, near.
```

### act1_battle — Чёрный лес
```text
A black spruce windfall forest at night, knee-deep mist, fallen trunks, straw charms and old bast shoes hanging from branches, a faint moon behind clouds in the upper left. Grey-green light.
```

### act2_battle — Мёртвая деревня
```text
A dead Slavic village at dusk: log huts with doors boarded up from the inside, a leaning well, crooked fences, rotten plank walkways leading into a swamp with pale will-o'-the-wisps. Sickly yellow light, flies in the air.
```

### act3_battle — Навь
```text
The Slavic underworld (Nav): an upside-down sky, the black river Smorodina with the red-hot Kalinov bridge in the distance, a forest of bones, quiet snow falling upward. Cold blue and dead white light.
```

### menu — главное меню
```text
A narrow path leading down into a black spruce forest at night, a cold blue-white glow far at the end of the path, mist, charms on the branches. Empty, quiet, ominous. Darker upper third for the game title.
```

## Иллюстрации карт — `assets/cards/<id>-gemini.png`, 1000×750 (4:3)

Шаблон:
```text
Card illustration, 4:3: <сюжет>. Focus on one clear action, dramatic composition, dark background fading to the edges so a card frame reads well. <мастер-стиль>
```

Эталоны редкостей (Богатырь):
- **обычная** `bogatyr_strike`: `The masked bogatyr brings his iron flanged mace down in a heavy overhead blow, mist tearing around the weapon.`
- **необычная** `bogatyr_challenge`: `The masked bogatyr slams his mace against his black oak shield and roars a challenge; dim shapes in the forest all turn toward him.`
- **редкая** `bogatyr_svyatogor`: `The masked bogatyr, battered and kneeling, rises with a single colossal counterblow; behind him looms the ghostly outline of a mountain-sized giant mirroring the strike.`
