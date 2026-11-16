# Иллюстрации карт — как собирается промпт

Файл-цель: `assets/cards/<id>-gemini.png`, 1000×750. После скачивания:
`python tools/cardart.py <id> [<id> ...]` (берёт свежие загрузки по порядку, режет до 4:3).

Запрос = **начало** + [**облик героя**] + **сюжет** + **хвост**.

## Начало

```text
Generate an image, landscape 4:3 aspect ratio. Card illustration:
```

## Облик героя (если герой в кадре)

| герой | текст |
| --- | --- |
| Богатырь | a huge grey-bearded warrior in hacked mail and an iron face mask, with a tall black oak shield carved with a stern face and a six-flanged mace |
| Ворожея | a gaunt woman in a damp linen shift, braids threaded with small bird bones, milky white eyes, rag dolls hanging on her belt, soot-stained fingers |
| Волхв | a tall old man in animal hides and a birch-bark mask, a long beard with woven charms, dark red rune marks painted on his arms, a staff of lightning-split oak |
| Волколак | a wiry hunter with a wolf pelt on his shoulders and a silver chain around his neck |
| Знахарка | a stooped middle-aged woman in a dark headscarf with old burn scars on her face, bundles of dried herbs on her back, a sickle and two clay jugs at her belt |
| Скоморох | a thin jester in a patchwork coat sewn with small bells, a white grinning mask on his face and three wooden masks on his belt |

## Сюжет

Одно предложение: кто что делает. Одно ясное действие, без свечения и частиц
(«кровь» — `dark red paint`). Для предметов и трав — `still life:` или `close-up of ...`.

Примеры:

- `bogatyr_guard` — … steps in front of a smaller hooded companion and plants his great shield in the mud to cover them both.
- `vol_thunder` — close-up of an old scarred hand carving a jagged lightning-shaped rune into a dark wooden post with a flint knife, fresh pale wood showing in the cut, shavings falling.
- `herb_wormwood` — still life: a bundle of silvery grey-green wormwood tied with twine lying on a dark scarred wooden table beside a small sickle, lit by one candle.

Сюжеты уже нарисованных карт по одной не записаны: список готовых — в `docs/manifest.json` (`done_in_bulk.cards`).

## Хвост

```text
Focus on one clear action, dramatic composition, dark background fading to the edges so a card frame reads well. No magic glow, no sparks or particles. Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk horror, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, realistic anatomy, readable silhouette. No text, no letters, no watermark, no frame, no modern objects.
```

## Лимит

У Gemini есть суточный лимит на изображения: 12.10.2026 он кончился после ~110 картинок
(44 спрайта врагов, 62 карты, пробы). Сообщение о лимите называет время сброса.

## Модель

В Gemini рисовать моделью Flash (не Flash-Lite: та дала плашку и слабую живопись). Метка модели — справа в поле ввода.

## Лимит, 13.10.2026

Flash и Pro кончились после ~215 картинок за день (сброс 13.10 в 17:07); остаётся только Flash-Lite — им не рисуем.
Третий набор: Богатырь нарисован весь (60), Ворожея — 48 из 60. Не нарисованы у Ворожеи:
`vor_thrice_cursed`, `vor_plague_mother`, `vor_last_rites`, `vor_bargain`, `vor_wailing`, `vor_death_mask`,
`vor_ill_star`, `vor_unmake`, `vor_witch_mark`, `vor_coven`, `vor_common_grief`, и весь третий набор Волхва,
Волколака, Знахарки, Скомороха и нейтральные.

## Конвейер в браузере

Чтобы не слать длинный текст в каждую вкладку, на `gemini.google.com` в `localStorage` лежат:
`tropaH` (начало), `tropaT` (хвост), `tropaB/V/O/W/Z/S` (облики героев), `tropaP` (JSON «id → сюжет»),
`tropaGo` (проверить модель, вставить, отправить), `tropaRun` (то же по id из `tropaP`), `tropaDl` (скачать последнюю картинку).
Цикл на четыре вкладки: клик по пустому месту → `eval(localStorage.tropaDl)()` → новая страница → `eval(localStorage.tropaRun)('<id>')`.
`tropaGo` возвращает `LITE` и ничего не отправляет, если выбрана Flash-Lite.

## Сюжеты третьего набора

Лежат файлами `docs/prompts/plots/<герой>3.json`: «id → сюжет». `_look` — буква облика героя (`tropaB/V/O/W/Z/S`),
он ставится перед сюжетом; сюжет с `@` в начале идёт без героя (предмет, зверь, крупный план).
В браузер файл попадает без перепечатки: `eval(localStorage.tropaInput)()` ставит на страницу своё поле выбора файла,
в него инструментом загрузки кладутся файлы, `await eval(localStorage.tropaLoad)()` разбирает их в `tropaP`.
Gemini эти файлы не получает: поле своё, после разбора убирается.
Сюжеты Богатыря и Ворожеи в файлы не записаны — они только в `tropaP`.

## Реликвии

`python tools/relic_prompts.py` пишет `docs/prompts/plots/relics.json` — полный запрос на каждую реликвию
(квадрат 1:1, один предмет на тёмном фоне). Что рисовать, берётся из английского названия; где названия мало —
из таблицы `WHAT` в самом скрипте. После скачивания: `python tools/relicart.py <id> ...` → `assets/relics/<id>-gemini.png`, 256×256.
В бою реликвии игрока стоят рядом под списком отряда; без картинки — первые буквы названия.

## Лимит — окно, а не сутки

13.10.2026: после сброса в 17:07 Flash кончился снова около 19:15, новый сброс назначен на 22:07 — то есть окно около пяти часов,
в него входит примерно 110–120 картинок. Четыре вкладки разом работают первые ~80 картинок, потом Gemini отклоняет отправку
(«Что-то пошло не так (1095)», текст остаётся в поле). Помогает пауза 15 минут и одна вкладка, одна карта за заход.
Очередь оставшихся id лежит в `localStorage.tropaQ`; `eval(localStorage.tropaNext)()` берёт следующую,
`eval(localStorage.tropaDl)()` скачивает и, если картинки нет, возвращает id в очередь. После «скачать» ждать 10 секунд до ухода со страницы.

## Google Flow

Когда у Gemini кончается лимит, рисуем в `flow.google.com` (Nano Banana 2.1): один запрос агенту — шесть картинок.
Запросы собирает `tools/flow_next.py`, принимает `tools/flowart.py` (`--match` сам подбирает картинку к id по названию файла),
проверять глазами — `tools/labelsheet.py` (лист с подписями). Места боёв описаны в `plots/places.json`:
`assets/bg/at_<враг>-gemini.png`, место выбирает первый враг встречи.
Скачивать только через редактор картинки (значок загрузки → 1K, стрелка вправо — следующая);
сохранять файлы скриптом со страницы нельзя — Chrome после этого блокирует все загрузки вкладки.

### heavy_eyelids (Kot Bayun's status card), repainted 2026-11-15

It had another card's picture (the healer blowing on a wolf). Asked in the Flow prompt box, cut
to 4:3 and saved at 1000×750:

> Card illustration for a dark Slavic folk-horror card game, landscape 4:3, one single image: a
> weary peasant warrior in a dim forest at night, his eyelids drooping shut against his will, head
> sagging, hand slipping from his axe, while out of the dark behind him two huge glowing yellow cat
> eyes watch and a faint drifting purr-like haze of grey sleep curls around his head like heavy
> smoke. (+ the style paragraph above)
