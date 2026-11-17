# Balance analytics

What the game writes down about every finished run, where it goes, and how to read it to see
which cards, relics, enemies, heroes and builds carry runs and which do not.

## What is recorded

The rules crate counts as the run is played (`crates/core/src/stats.rs`, kept in `Run::stats`,
so it survives saves and travels with the party state):

- **Per player:** every card played (times, damage, block), damage dealt and taken, block,
  healing, times downed, potions drunk, gold gained and spent, every deck change (added,
  upgraded, removed; act and floor) and every relic gained (and from where).
- **Per fight:** act, floor, kind (fight, elite, boss), the foes, won or not, rounds, HP each hero
  lost, damage by foe kind, heroes downed, cards played, potions drunk.
- **Every offer:** each card, relic and potion shown in a reward, the shop, a chest or a trophy,
  to whom, whether it was taken, and the price in the shop. This is what tells what keeps
  appearing but is passed over.
- **Omens:** which choice was made at each omen.

When the run ends (won or lost), `crates/game/src/analytics.rs` adds the end state: every
player's deck, relics, potions, HP and gold, the foe that ended the run, and the player's
**build**.

### Builds

Each card belongs to one or more *families* by what it does (`family_of`): rot, bleed, hex,
fear, debuff, strength, thorns, valor, block, runes, discard, exhaust, draw, energy, heal, brew,
masks, shift, drowse, pike, echo, aoe, multi_hit, retain, coop, powers. Starting cards, curses
and statuses do not count. A family held by a quarter of the remaining cards is a lead; the
build is the one or two strongest leads joined with `+` (draw, energy, block and powers support
every deck and never name one), or `mixed`. So a Hunter deck full of bleed cards reads
`bleed`, a Volkhv deck of runes and hex `runes+hex`.

A new card mechanic needs a family: add it in `family_of`. The test
`every_reward_card_has_a_family` fails when more than a tenth of the cards have none.

## Where it goes

1. A line in `<user dir>/profiles/analytics.jsonl` (the in-game screen Chronicle → Analytics reads
   it).
2. The same line in `profiles/analytics_pending.jsonl`, then posted in a background thread to the
   sheet's web app. A line that fails stays in the pending file and is tried again at the next
   start. The sheet ignores a run it already has, so a retry never doubles a row.

In a party only the **host** sends, once for everyone; LAN and Steam parties are sent like solo
runs. Only the dev autopilot's own runs (`bot_me`) are left out; a dev session with autopilot
allies is sent with mode `bots`.

The address is `telemetry_url` in `assets/config.ron`, or the `TROPA_TELEMETRY_URL` environment
variable, which wins. Empty means nothing is sent; the local file is still written.

## The sheet

`tools/google_sheets_telemetry.js` is the whole server side: a Google Apps Script bound to a
Google Sheet.

### Setting it up (or redeploying after a change)

1. Open the sheet → Extensions → Apps Script. Paste the file over `Code.gs`, save.
2. Deploy → New deployment → type *Web app*; execute as *Me*, access *Anyone*. Copy the `/exec`
   address into `telemetry_url`. Changing the code later: Deploy → Manage deployments → edit →
   *New version* (the address stays the same; saving the code alone changes nothing live).
3. Reload the sheet: an «Аналитика» menu appears. Run «Пересчитывать каждый час» once (it asks
   for permission) so the summaries refresh by themselves.
4. Check it is alive: open the `/exec` address in a browser, it answers
   `{"ok":true,"sheets":[…]}`.

### Raw sheets (written by each run)

| Sheet | One row per |
|---|---|
| Забеги | run: version, mode, result, circle, act, floors, elites, bosses, heroes, killed_by |
| Игроки | player in a run: hero, build, families, deck, relics, potions, damage, block, healing, gold |
| Бои | fight: act, floor, kind, foes, won, rounds, HP lost by the party, downed, damage by foe |
| Предложения | card, relic or potion offered: to which hero, from where, taken or not, price |
| Карты_игра | card per player per run: times played, damage, block |

### Summary sheets (rebuilt by «Пересчитать»)

- **Карты:** offered, taken, *pick_rate*, bought, in how many final decks, *win_rate* of those
  decks and *win_delta* against all players' win rate, plays per deck, damage and block per play.
  Often offered with a low pick rate: nobody wants it. Taken often with a negative win delta: a
  trap. Rarely played though held: dead weight in hand.
- **Реликвии:** offered, taken, pick rate, held, win rate and delta.
- **Враги:** fights, how many of those ended the run, times it struck the last blow, average HP
  each hero lost, damage each hero took from it, rounds.
- **Встречи:** the same per encounter (kind plus the set of foes).
- **Герои / Сборки:** runs, wins, win rate and delta, damage dealt and taken, deck size; per hero
  and per hero with build. A build that is common but loses is a planned build that does not
  work; a build that never shows up was never offered enough cards to form.

Small numbers lie: read a rate only once its row has a few dozen runs behind it.

## In the game

Chronicle → Analytics shows the local history: runs and wins, the cards offered most and taken
least, and the builds by hero with their win rate.
