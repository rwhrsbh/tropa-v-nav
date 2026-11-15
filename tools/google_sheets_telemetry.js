/**
 * «Тропа в Навь» — the balance sheet. Google Apps Script bound to a Google Sheet.
 *
 * The game posts one JSON record per finished run (crates/game/src/analytics.rs, `RunRecord`,
 * schema 2). doPost only appends raw rows; the summary sheets are rebuilt from the raw ones by
 * `rebuild()` (menu «Аналитика → Пересчитать», and an hourly trigger made by `installTrigger()`).
 * Setup and how to read the sheets: docs/telemetry_and_analytics.md.
 *
 * Raw sheets (one row per…):
 *   Забеги       run             Игроки       player in a run
 *   Бои          fight           Предложения  card/relic/potion offered (taken or not)
 *   Карты_игра   card per player per run (plays, damage, block)
 * Summary sheets: Карты, Реликвии, Враги, Встречи, Герои, Сборки.
 */

const RAW = {
  'Забеги': ['run_id', 'time', 'version', 'mode', 'result', 'circle', 'act', 'floors', 'elites', 'bosses', 'players', 'heroes', 'killed_by'],
  'Игроки': ['run_id', 'result', 'mode', 'circle', 'hero', 'build', 'families', 'hp', 'max_hp', 'gold', 'deck_size', 'deck', 'relics', 'potions',
    'damage_dealt', 'damage_taken', 'block_gained', 'healed', 'downed', 'gold_gained', 'gold_spent', 'potions_used'],
  'Бои': ['run_id', 'result', 'circle', 'act', 'floor', 'kind', 'enemies', 'won', 'rounds', 'hp_lost', 'downed', 'cards_played', 'potions_used', 'party', 'damage_by'],
  'Предложения': ['run_id', 'result', 'circle', 'act', 'floor', 'hero', 'kind', 'source', 'id', 'taken', 'price'],
  'Карты_игра': ['run_id', 'result', 'hero', 'build', 'card', 'played', 'damage', 'block'],
};

function doGet() {
  return json({ ok: true, sheets: Object.keys(RAW) });
}

function doPost(e) {
  const lock = LockService.getScriptLock();
  lock.waitLock(30000);
  try {
    const r = JSON.parse(e.postData.contents);
    if (!r.run_id) return json({ error: 'no run_id' });
    const ss = SpreadsheetApp.getActiveSpreadsheet();
    const runs = sheet(ss, 'Забеги');
    // the same run sent twice (a retry after a lost answer) is written once
    const seen = runs.getLastRow() > 1 ? runs.getRange(2, 1, runs.getLastRow() - 1, 1).getValues().flat() : [];
    if (seen.indexOf(r.run_id) >= 0) return json({ ok: true, duplicate: true });
    appendRows(ss, 'Забеги', [[r.run_id, new Date(r.timestamp * 1000), r.version, r.mode, r.result, r.circle, r.act, r.floors, r.elites, r.bosses,
      r.players.length, r.players.map(p => p.hero).join(','), r.killed_by || '']]);
    const st = r.stats || {};
    const ps = st.players || [];
    appendRows(ss, 'Игроки', r.players.map((p, i) => {
      const s = ps[i] || {};
      return [r.run_id, r.result, r.mode, r.circle, p.hero, p.build, JSON.stringify(p.families || {}), p.hp, p.max_hp, p.gold, p.deck.length,
        p.deck.join(','), p.relics.join(','), p.potions.join(','), s.damage_dealt || 0, s.damage_taken || 0, s.block_gained || 0, s.healed || 0,
        s.downed || 0, s.gold_gained || 0, s.gold_spent || 0, JSON.stringify(s.potions_used || {})];
    }));
    appendRows(ss, 'Бои', (st.fights || []).map(f => [r.run_id, r.result, r.circle, f.act, f.floor, f.kind, f.enemies.join(','), f.won, f.rounds,
      sum(f.hp_lost || []), f.downed, f.cards_played, f.potions_used, r.players.length, JSON.stringify(f.damage_by || {})]));
    appendRows(ss, 'Предложения', (st.offers || []).map(o => [r.run_id, r.result, r.circle, o.act, o.floor, (r.players[o.player] || {}).hero || '',
      o.kind, o.source, o.id, o.taken, o.price == null ? '' : o.price]));
    const uses = [];
    ps.forEach((s, i) => Object.keys(s.cards || {}).forEach(c => {
      const u = s.cards[c];
      uses.push([r.run_id, r.result, s.hero, (r.players[i] || {}).build || '', c, u.played, u.damage || 0, u.block || 0]);
    }));
    appendRows(ss, 'Карты_игра', uses);
    return json({ ok: true });
  } catch (err) {
    return json({ error: String(err) });
  } finally {
    lock.releaseLock();
  }
}

function onOpen() {
  SpreadsheetApp.getUi().createMenu('Аналитика').addItem('Пересчитать', 'rebuild').addItem('Пересчитывать каждый час', 'installTrigger').addToUi();
}

function installTrigger() {
  ScriptApp.getProjectTriggers().filter(t => t.getHandlerFunction() === 'rebuild').forEach(t => ScriptApp.deleteTrigger(t));
  ScriptApp.newTrigger('rebuild').timeBased().everyHours(1).create();
}

/** Rebuilds every summary sheet from the raw ones. */
function rebuild() {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  const players = rows(ss, 'Игроки');
  const fights = rows(ss, 'Бои');
  const offers = rows(ss, 'Предложения');
  const uses = rows(ss, 'Карты_игра');
  const runs = rows(ss, 'Забеги');
  const base = rate(players.filter(p => p.result === 'won').length, players.length);

  // cards: offered, taken, pick rate, win rate of decks holding it against the baseline, plays
  const cards = {};
  const card = id => cards[id] || (cards[id] = { offered: 0, taken: 0, decks: 0, won: 0, played: 0, damage: 0, block: 0, bought: 0 });
  offers.filter(o => o.kind === 'card').forEach(o => {
    const c = card(o.id);
    c.offered++;
    if (o.taken) { c.taken++; if (o.source === 'shop') c.bought++; }
  });
  players.forEach(p => uniq(String(p.deck).split(',').filter(Boolean).map(c => c.replace(/\+$/, ''))).forEach(id => {
    const c = card(id);
    c.decks++;
    if (p.result === 'won') c.won++;
  }));
  uses.forEach(u => {
    const c = card(String(u.card).replace(/\+$/, ''));
    c.played += +u.played;
    c.damage += +u.damage;
    c.block += +u.block;
  });
  write(ss, 'Карты', ['card', 'offered', 'taken', 'pick_rate', 'bought', 'in_decks', 'win_rate', 'win_delta', 'plays', 'plays_per_deck', 'dmg_per_play', 'block_per_play'],
    Object.keys(cards).map(id => {
      const c = cards[id];
      const wr = rate(c.won, c.decks);
      return [id, c.offered, c.taken, rate(c.taken, c.offered), c.bought, c.decks, wr, c.decks ? wr - base : '', c.played,
        c.decks ? c.played / c.decks : '', c.played ? c.damage / c.played : '', c.played ? c.block / c.played : ''];
    }).sort((a, b) => b[1] - a[1]));

  // relics: offered and taken from offers, win rate of the players who held it
  const relics = {};
  const relic = id => relics[id] || (relics[id] = { offered: 0, taken: 0, held: 0, won: 0 });
  offers.filter(o => o.kind === 'relic').forEach(o => { const r = relic(o.id); r.offered++; if (o.taken) r.taken++; });
  players.forEach(p => uniq(String(p.relics).split(',').filter(Boolean)).forEach(id => { const r = relic(id); r.held++; if (p.result === 'won') r.won++; }));
  write(ss, 'Реликвии', ['relic', 'offered', 'taken', 'pick_rate', 'held', 'win_rate', 'win_delta'],
    Object.keys(relics).map(id => { const r = relics[id]; const wr = rate(r.won, r.held); return [id, r.offered, r.taken, rate(r.taken, r.offered), r.held, wr, r.held ? wr - base : '']; })
      .sort((a, b) => b[4] - a[4]));

  // enemies and encounters: how often met, how often they ended a run, hp they cost
  const foes = {};
  const encounters = {};
  fights.forEach(f => {
    const party = +f.party || 1;
    const list = String(f.enemies).split(',').filter(Boolean);
    const key = f.kind + ': ' + list.slice().sort().join(' + ');
    const e = encounters[key] || (encounters[key] = { n: 0, lost: 0, hp: 0, rounds: 0, downed: 0 });
    e.n++; e.hp += f.hp_lost / party; e.rounds += +f.rounds; e.downed += +f.downed; if (!f.won) e.lost++;
    let dmg = {};
    try { dmg = JSON.parse(f.damage_by || '{}'); } catch (_) {}
    uniq(list).forEach(id => {
      const o = foes[id] || (foes[id] = { n: 0, lost: 0, hp: 0, dmg: 0, rounds: 0 });
      o.n++; o.hp += f.hp_lost / party; o.rounds += +f.rounds; o.dmg += (dmg[id] || 0) / party; if (!f.won) o.lost++;
    });
  });
  const killers = {};
  runs.filter(r => r.result === 'lost' && r.killed_by).forEach(r => { killers[r.killed_by] = (killers[r.killed_by] || 0) + 1; });
  write(ss, 'Враги', ['enemy', 'fights', 'run_ended', 'killed_by', 'avg_hp_lost', 'avg_damage', 'avg_rounds'],
    Object.keys(foes).map(id => { const o = foes[id]; return [id, o.n, o.lost, killers[id] || 0, o.hp / o.n, o.dmg / o.n, o.rounds / o.n]; })
      .sort((a, b) => b[4] - a[4]));
  write(ss, 'Встречи', ['encounter', 'fights', 'run_ended', 'avg_hp_lost', 'avg_rounds', 'downed_per_fight'],
    Object.keys(encounters).map(k => { const e = encounters[k]; return [k, e.n, e.lost, e.hp / e.n, e.rounds / e.n, e.downed / e.n]; })
      .sort((a, b) => b[3] - a[3]));

  // heroes and builds
  const group = (keyOf, name, sheetName) => {
    const g = {};
    players.forEach(p => {
      const k = keyOf(p);
      const x = g[k] || (g[k] = { n: 0, won: 0, floors: 0, dealt: 0, taken: 0, size: 0 });
      x.n++; if (p.result === 'won') x.won++; x.dealt += +p.damage_dealt; x.taken += +p.damage_taken; x.size += +p.deck_size;
    });
    write(ss, sheetName, name.concat(['runs', 'won', 'win_rate', 'win_delta', 'avg_damage_dealt', 'avg_damage_taken', 'avg_deck_size']),
      Object.keys(g).map(k => { const x = g[k]; const wr = rate(x.won, x.n); return k.split(' | ').concat([x.n, x.won, wr, wr - base, x.dealt / x.n, x.taken / x.n, x.size / x.n]); })
        .sort((a, b) => b[name.length] - a[name.length]));
  };
  group(p => p.hero, ['hero'], 'Герои');
  group(p => p.hero + ' | ' + p.build, ['hero', 'build'], 'Сборки');
}

// --- helpers ---

function json(o) {
  return ContentService.createTextOutput(JSON.stringify(o)).setMimeType(ContentService.MimeType.JSON);
}

function sheet(ss, name) {
  let s = ss.getSheetByName(name);
  if (!s) {
    s = ss.insertSheet(name);
    if (RAW[name]) { s.appendRow(RAW[name]); s.setFrozenRows(1); }
  }
  return s;
}

function appendRows(ss, name, list) {
  if (!list.length) return;
  const s = sheet(ss, name);
  s.getRange(s.getLastRow() + 1, 1, list.length, list[0].length).setValues(list);
}

function rows(ss, name) {
  const s = ss.getSheetByName(name);
  if (!s || s.getLastRow() < 2) return [];
  const v = s.getDataRange().getValues();
  const head = v.shift();
  return v.map(r => { const o = {}; head.forEach((h, i) => { o[h] = r[i]; }); return o; });
}

function write(ss, name, head, list) {
  const s = sheet(ss, name);
  s.clearContents();
  s.getRange(1, 1, 1, head.length).setValues([head]).setFontWeight('bold');
  s.setFrozenRows(1);
  if (list.length) s.getRange(2, 1, list.length, head.length).setValues(list.map(r => r.map(x => (typeof x === 'number' && !Number.isInteger(x)) ? Math.round(x * 100) / 100 : x)));
}

function rate(a, b) { return b ? a / b : 0; }
function sum(a) { return a.reduce((x, y) => x + y, 0); }
function uniq(a) { return a.filter((x, i) => a.indexOf(x) === i); }
