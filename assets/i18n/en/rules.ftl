# Phrases that card text is assembled from.

fx-damage = Deal { $n } damage.
fx-damage-AllFoes = Deal { $n } damage to all enemies.
fx-damage-RandomFoe = Deal { $n } damage to a random enemy.
fx-hits = Deal { $n } damage { $k } times.
fx-hits-AllFoes = Deal { $n } damage to all enemies { $k } times.
fx-hits-RandomFoe = Deal { $n } damage to a random enemy { $k } times.
fx-block = Gain { $n } Block.
fx-block-Target = Give an ally { $n } Block.
fx-block-AllFriends = All allies gain { $n } Block.
fx-block-OtherFriends = Other allies gain { $n } Block.
fx-guard = Guard { $n }.
fx-apply = Apply { $n } { $s }.
fx-apply-Me = Gain { $n } { $s }.
fx-apply-AllFoes = Apply { $n } { $s } to all enemies.
fx-apply-RandomFoe = Apply { $n } { $s } to a random enemy.
fx-apply-AllFriends = All allies gain { $n } { $s }.
fx-apply-OtherFriends = Other allies gain { $n } { $s }.
fx-remove = Remove { $s } from the target.
fx-remove-Me = Lose all { $s }.
fx-consume = Respond: consume the target's { $s }.
fx-consume-AllFoes = Respond: consume every enemy's { $s }.
fx-cleanse = Remove every harmful status from an ally.
fx-cleanse-Me = Remove every harmful status from yourself.
fx-cleanse-AllFriends = Remove every harmful status from all allies.
fx-draw = Draw { $n ->
    [one] { $n } card
   *[other] { $n } cards
}.
fx-energy = Gain { $n } Energy.
fx-energy-Target = An ally gains { $n } Energy.
fx-energy-AllFriends = All allies gain { $n } Energy.
fx-energy-OtherFriends = Other allies gain { $n } Energy.
fx-heal = Heal { $n } HP.
fx-heal-Target = Heal an ally for { $n } HP.
fx-heal-AllFriends = Heal all allies for { $n } HP.
fx-losehp = Lose { $n } HP.
fx-losehp-Target = The target loses { $n } HP.
fx-revive = Raise a fallen ally with { $n }% HP.
fx-addcard-Hand = { $n ->
    [one] Add "{ $c }" to your hand.
   *[other] Add { $n } × "{ $c }" to your hand.
}
fx-addcard-Draw = { $n ->
    [one] Shuffle "{ $c }" into your deck.
   *[other] Shuffle { $n } × "{ $c }" into your deck.
}
fx-addcard-Discard = { $n ->
    [one] Put "{ $c }" into your discard pile.
   *[other] Put { $n } × "{ $c }" into your discard pile.
}
fx-addcard-Discard-Target = { $n ->
    [one] The target gets "{ $c }" in their discard pile.
   *[other] The target gets { $n } × "{ $c }" in their discard pile.
}
fx-taunt = Challenge: this round every enemy strikes only you.
fx-if = { $c }: { $then }
fx-else = Otherwise: { $else }
fx-repeat = { $n } times: { $body }
rune-pushed-out = Discharges when pushed out.
fx-rune = Carve a { $r } rune on an ally.
fx-rune-Me = Carve a { $r } rune.
fx-invoke = Invoke: all runes fire.
fx-shift = Shift.
fx-mask = Put on the { $m } mask.

amount-consumed = amount consumed
amount-played = cards played this round

cond-has-Me = If you have { $n ->
    [1] { $s }
   *[other] { $s } { $n }+
}
cond-has-Target = If the target has { $n ->
    [1] { $s }
   *[other] { $s } { $n }+
}
cond-has-AllFoes = If all enemies have { $n ->
    [1] { $s }
   *[other] { $s } { $n }+
}
cond-link = Link ({ $t })
cond-fatal = If fatal
cond-form-Human = As a man
cond-form-Beast = As the beast
cond-mask-Bear = In the Bear mask
cond-mask-Goat = In the Goat mask
cond-mask-Mara = In the Mara mask
cond-coop = In a party
cond-not = If not ({ $c })

trigger-CombatStart = At the start of combat: { $fx }
trigger-TurnStart = At the start of your turn: { $fx }
trigger-TurnEnd = At the end of your turn: { $fx }
trigger-CardPlayed = Whenever you play a card: { $fx }
trigger-AttackPlayed = Whenever you play an Attack: { $fx }
trigger-AllyHpLoss = Whenever an ally loses HP: { $fx }
trigger-Kill = Whenever you kill an enemy: { $fx }
trigger-Shifted = Whenever you shift: { $fx }
trigger-MaskChanged = Whenever you change masks: { $fx }
trigger-AllyRaised = Whenever an ally is raised: { $fx }
trigger-InHand = At the end of your turn in hand: { $fx }

type-Attack = Attack
type-Skill = Skill
type-Power = Power
type-Herb = Herb
type-Curse = Curse
type-Status = Status

kw-Exhaust = Exhaust
kw-Exhaust-help = Removed from your deck until the end of combat when played.
kw-Retain = Retain
kw-Retain-help = Not discarded at the end of your turn.
kw-Ethereal = Decay
kw-Ethereal-help = If not played, it is gone at the end of your turn.
kw-Unplayable = Unplayable
kw-Unplayable-help = This card cannot be played.
kw-Guard = Guard
kw-Guard-help = Block for an ally. Damage that gets through their Block hits the Bogatyr instead until the round ends.
kw-Taunt = Challenge
kw-Taunt-help = Every enemy that was aiming at a hero attacks the Bogatyr this round.
kw-Link = Link
kw-Link-help = Bonus if an ally has already played a card of that type this round.
kw-Invoke = Invoke
kw-Invoke-help = Fires every rune on every hero at once. The runes stay.
kw-Shift = Shift
kw-Shift-help = The Volkolak changes form. Every card in hand flips.

status-Strength = Strength
status-Strength-help = Attacks deal +X damage.
status-TempStrength = Strength this turn
status-TempStrength-help = Attacks deal +X damage until the round ends.
status-Fortitude = Fortitude
status-Fortitude-help = Cards give +X Block.
status-Vulnerable = Sundered
status-Vulnerable-help = Takes 50% more damage. Drops by 1 each round.
status-Weak = Muted
status-Weak-help = Deals 25% less damage. Drops by 1 each round.
status-Rot = Rot
status-Rot-help = Takes X damage at the end of its turn, then drops by 1.
status-Hex = Evil Eye
status-Hex-help = The next X hits on the target deal +3 damage, from any hero.
status-Bleed = Bleed
status-Bleed-help = Takes X damage every time it attacks.
status-Fear = Fear
status-Fear-help = Chance to skip its next attack: as many percent as it has Fear. 75 at most.
status-Thorns = Thorns
status-Thorns-help = Deals X damage back to attackers.
status-HuntMark = Hunter's Trail
status-HuntMark-help = All heroes deal +X damage to this target until the round ends.
status-Valor = Valor
status-Valor-help = Builds when the Bogatyr soaks damage: 1 per 5. Spent by finishers.
status-Stun = Stun
status-Stun-help = Skips its turn.
status-Mist = Mist
status-Mist-help = The next attack on the bearer deals half damage.

rune-Thunder = Thunder
rune-Thunder-help = End of turn: 6 damage to a random enemy.
rune-Frost = Frost
rune-Frost-help = End of turn: the bearer gains 4 Block.
rune-Mist = Mist
rune-Mist-help = End of turn: the next attack on the bearer deals half damage.
rune-Flame = Svarog's Flame
rune-Flame-help = End of turn: 3 damage to all enemies.
rune-Zhiva = Zhiva
rune-Zhiva-help = End of turn: heals the bearer for 2.

mask-Bear = Bear
mask-Bear-help = Attacks deal +3 damage.
mask-Goat = Goat
mask-Goat-help = Every card you play gives a random ally 2 Block.
mask-Mara = Mara
mask-Mara-help = Every blow applies 7.5% Fear.

fx-consumecurses = Respond: consume every Curse on the target.
fx-once = Once per combat: { $body }
fx-onceturn = Once per turn: { $body }
fx-every = Every { $k }th time: { $body }
fx-everyturn = Every { $k }th time in a turn: { $body }
fx-maxhp = +{ $n } max HP.
fx-echo = Echo: replay the last card an ally played this round, for free.
fx-echo-AllFriends = Echo for all: every hero replays the last card an ally played this round, for free.
fx-herb = Into the Cauldron: { $h }.
fx-brew = Brew everything in the Cauldron.
fx-provoke = This enemy now strikes you.
amount-curses = Curse on the target
status-Chained = Chained
status-Chained-help = The Volkolak cannot change form.

flag-mor = Rot on enemies triggers twice and does not fade.
flag-storm = Thunder runes strike all enemies.
flag-free_rune = Your next rune is free.
flag-no_hunger = Hunger has no effect in Beast form.
flag-foresight = You see enemy intents 2 rounds ahead.
flag-brew_all = Your next brew affects all allies or all enemies.
flag-first_rune_twice = Your first rune fires twice.
flag-big_cauldron = The Cauldron holds 4 herbs.

hook-transfer = Strip every harmful status from the heroes and lay it on this enemy.
hook-kolyada = X times: a random rune fires on every hero.
hook-balagan = Enemies swap targets among themselves.
hook-misdirect = This enemy's next attack on a hero hits another enemy instead.
hook-random_herb = Add a random herb to your hand.

kw-Curse = Curse
kw-Curse-help = Rot, Evil Eye and Muted together. Respond cards consume it for a bonus.
kw-Echo = Echo
kw-Echo-help = A free copy of the last card an ally played this round; it is gone once played. Alone, it copies your own last card.
kw-Brew = Brew
kw-Brew-help = Herbs stay in the Cauldron between turns. Brewing turns them into an effect by recipe. When the cauldron is full, a new herb spills the oldest, which takes weak effect.

form-Human = Human
form-Beast = Beast
res-moon = Moon
res-mask = Mask
res-runes = Runes
res-cauldron = Cauldron

herb-wormwood = Wormwood
herb-sleep = Sleep-grass
herb-weep = Weeping-grass
herb-burst = Burst-grass
herb-fern = Fern Flower
kw-Passable = Passable
kw-Passable-help = Drag the card onto an ally to hand it over for 1 Energy.
trigger-SkillPlayed = Whenever you play a Skill: { $fx }
trigger-Hurt = Whenever an enemy wounds you: { $fx }

kw-Rune = Rune
kw-Rune-help = A rune fires at the end of every turn. Five fit on you, three on an ally: a new one pushes out the oldest, which fires one last time.
rune-Thunder-short = 6 damage to a random foe each turn
rune-Frost-short = 4 Block each turn
rune-Mist-short = halves the next attack each turn
rune-Flame-short = 3 damage to all foes each turn
rune-Zhiva-short = heals 2 each turn
rune-discharged = { $r } discharges
herb-spilled = { $h } boils over
herb-wormwood-help = Brews into damage to all foes. Spilled: 2 damage to all foes.
herb-sleep-help = Brews into Block. Spilled: 3 Block.
herb-weep-help = Brews into healing. Spilled: heals 2.
herb-burst-help = Brews into one great blow at a target. Spilled: 5 damage to a random foe.
herb-fern-help = Stands in for any herb and doubles the brew. Spilled: draw a card.
trait-marked-help = Every aimed blow of the foes goes to this hero.
trait-blind-help = This turn the hero cannot see what their cards will do.
trait-coiled-help = No attacks until the serpent takes 15 damage or this turn ends.
trait-confused-help = This turn an aimed card finds a random foe.
trait-mired-help = No cards this turn. An ally's card played on them pulls them out; otherwise 8 HP lost as the turn ends.
trait-asleep-help = No cards this turn.
trait-charmed-help = The next attack turns on a friend, or is wasted when alone.
trait-frozen-help = The first cards in hand cannot be played this turn.
trait-winter-help = One card fewer each turn, up to three, while the ice crown stands.
trait-immortal-help = Cannot die while the needle is whole.
trait-draw_less = The earth shakes
trait-draw_less-help = Next turn, as many cards fewer as the badge shows.
trait-energy_less = Dragged under
trait-energy_less-help = One less energy next turn.
trait-no_intents = Sirin's song
trait-no_intents-help = This turn the foes' intents cannot be seen.
herb-wormwood-spill = 2 damage to all foes
herb-sleep-spill = 3 Block
herb-weep-spill = heal 2
herb-burst-spill = 5 damage to a random foe
herb-fern-spill = draw a card
hook-pass_fever = The fever passes to the next hero.
battle-missed = Miss!
battle-gold-stolen = −{ $n } gold
battle-gold-returned = +{ $n } gold
battle-fled = Fled!
battle-gold-scattered = Scattered { $n } gold
card-flip-hint = Right click: turn over — upgraded or as it is
trait-dodge = Elusive
trait-dodge-help = The first attack against it each turn misses.
trait-flying = Flying
trait-flying-help = Aimed blows cannot reach it: only blows at all foes, and runes.
trait-hiding = Hiding
trait-hiding-help = Cannot be aimed at while other foes stand.
trait-enrage = Rage
trait-enrage-help = Gains 2 Strength from each attack that wounds it.
trait-lifesteal = Bloodsucker
trait-lifesteal-help = Heals from each wound its attacks deal.
trait-piercing = Through the shield
trait-piercing-help = Only half of your block stops its attacks.
trait-rises = Rises again
trait-rises-help = The first time it falls, it gets up with half its health.
trait-hunter = Hunter
trait-hunter-help = Strikes whoever holds the needle, or has hurt the chain of its death the most.
status-Drowse = Drowse
status-Drowse-help = Gathered by doing nothing: each energy unspent at the end of a turn gives 1 Drowse. Yemelya's cards spend it, and some are paid for with it.
amount-missing = { $pct }% of health lost
fx-damage-WeakestFoe = Deal { $n } damage to the weakest enemy.
fx-apply-WeakestFoe = Apply { $n } { $s } to the weakest enemy.
hook-spread_curse = This foe's Rot, Hex and Weak land on all the others too.
hook-fire_all_runes = All your runes go off now and stay carved.
hook-spill_cauldron = Empty the cauldron: each herb gives the whole party 3 block.
hook-steal_block = Take all of the foe's block for yourself.
trait-pack = Pack
trait-pack-help = Bites harder for each other foe beside it.
trait-spores = Spores
trait-spores-help = On death, gives all heroes 3 Rot.
trait-vengeful = Vengeful
trait-vengeful-help = On death, gives the other foes 1 Strength.
trait-burning = Burning
trait-burning-help = When killed, explodes: 8 damage to all heroes.
trait-greedy = Greedy
trait-greedy-help = Steals gold: kill it fast.
trait-hollow = Straw
trait-hollow-help = One blow takes at most 6 from it: strike many times.
trait-growing = Growing Angry
trait-growing-help = Stronger with every blow.
trait-doom = Doom
trait-doom-help = When the count reaches zero, the hero loses 30 HP. Doom lifts if Morovitsa is killed.
trait-kinetic_moss = Kinetic Moss
trait-kinetic_moss-help = Attacks under 6 damage deal it only 1 and store a kinetic charge.
trait-kinetic_charge = Kinetic Charge
trait-kinetic_charge-help = Each charge adds 3 damage to its Kinetic Burst.
trait-bayun_perch = Bayun's Perch
trait-bayun_perch-help = Perched on a pillar, he lashes back: every attack hit on him deals 1 to the attacker. Multi-hit cards hurt you too.
trait-spell_mirror = Spell Mirror
trait-spell_mirror-help = Every attack hit on her returns 30% of its damage to the attacker (at least 1).
trait-flicker = Shadow Flicker
trait-flicker-help = On even rounds no attack deals her more than 1 damage.
trait-gorynych_head = Dragon Head
trait-gorynych_head-help = As long as one head lives, severed heads regrow after 2 turns.
trait-yaga_mortar = Yaga's Mortar
trait-yaga_mortar-help = While the mortar holds, Yaga takes 35% less damage. It breaks if she takes 28 damage in a single turn.
trait-overheat = Overheat
trait-overheat-help = Accumulates searing heat from fiery attacks.
battle-maimed = Severed!
battle-beheaded = headless
trait-lost-one = A limb lost
trait-lost-one-help = For the rest of the run, 1 card fewer each turn.
trait-lost-both = Both limbs lost
trait-lost-both-help = For the rest of the run, no attacks and no block, and 1 card and 1 energy fewer each turn.
battle-walks-headless = walks headless
trait-walks-headless = Walks headless
trait-walks-headless-help = Water raised the body but sewed nothing. A tenth of their health, until the Graveyard Seamstress gives the head back.
trait-lost-weapon = Weapon arm lost
trait-lost-weapon-help = For the rest of the run, no attacks, and 1 card fewer each turn.
trait-lost-shield = Shield arm lost
trait-lost-shield-help = For the rest of the run, no cards that give block, and 1 card fewer each turn.
trait-lost-weapon-bogatyr = Mace arm lost
trait-lost-weapon-bogatyr-help = Until the run ends or it is sewn back: no attacks. 1 card fewer each turn.
trait-lost-shield-bogatyr = Shield arm lost
trait-lost-shield-bogatyr-help = Until the run ends or it is sewn back: no block. 1 card fewer each turn.
trait-lost-both-bogatyr = Both arms lost
trait-lost-both-bogatyr-help = Until the run ends or they are sewn back: no attacks; no block. 1 card and 1 energy fewer each turn.
trait-lost-weapon-vorozheya = Bone hand lost
trait-lost-weapon-vorozheya-help = Until the run ends or it is sewn back: no attacks, and no turning a hex into power. 1 card fewer each turn.
trait-lost-shield-vorozheya = Doll arm lost
trait-lost-shield-vorozheya-help = Until the run ends or it is sewn back: no block and no hexes. 1 card fewer each turn.
trait-lost-both-vorozheya = Both arms lost
trait-lost-both-vorozheya-help = Until the run ends or they are sewn back: no attacks, and no turning a hex into power; no block and no hexes. 1 card and 1 energy fewer each turn.
trait-lost-weapon-volhv = Staff arm lost
trait-lost-weapon-volhv-help = Until the run ends or it is sewn back: no attacks and no calling down runes. 1 card fewer each turn.
trait-lost-shield-volhv = Carved arm lost
trait-lost-shield-volhv-help = Until the run ends or it is sewn back: no block and no cutting runes. 1 card fewer each turn.
trait-lost-both-volhv = Both arms lost
trait-lost-both-volhv-help = Until the run ends or they are sewn back: no attacks and no calling down runes; no block and no cutting runes. 1 card and 1 energy fewer each turn.
trait-lost-weapon-volkolak = Knife arm lost
trait-lost-weapon-volkolak-help = Until the run ends or it is sewn back: no attacks as a man; the beast bites all the same. 1 card fewer each turn.
trait-lost-shield-volkolak = Other arm lost
trait-lost-shield-volkolak-help = Until the run ends or it is sewn back: no block. 1 card fewer each turn.
trait-lost-both-volkolak = Both arms lost
trait-lost-both-volkolak-help = Until the run ends or they are sewn back: no attacks as a man; the beast bites all the same; no block. 1 card and 1 energy fewer each turn.
trait-lost-weapon-znaharka = Jug arm lost
trait-lost-weapon-znaharka-help = Until the run ends or it is sewn back: no attacks and no brewing. 1 card fewer each turn.
trait-lost-shield-znaharka = Basket arm lost
trait-lost-shield-znaharka-help = Until the run ends or it is sewn back: no block and no herbs in the cauldron. 1 card fewer each turn.
trait-lost-both-znaharka = Both arms lost
trait-lost-both-znaharka-help = Until the run ends or they are sewn back: no attacks and no brewing; no block and no herbs in the cauldron. 1 card and 1 energy fewer each turn.
trait-lost-weapon-skomoroh = Tambourine arm lost
trait-lost-weapon-skomoroh-help = Until the run ends or it is sewn back: no attacks. 1 card fewer each turn.
trait-lost-shield-skomoroh = Free hand lost
trait-lost-shield-skomoroh-help = Until the run ends or it is sewn back: no block and no changing masks. 1 card fewer each turn.
trait-lost-both-skomoroh = Both arms lost
trait-lost-both-skomoroh-help = Until the run ends or they are sewn back: no attacks; no block and no changing masks. 1 card and 1 energy fewer each turn.
trait-lost-weapon-emelya = Bucket arm lost
trait-lost-weapon-emelya-help = Until the run ends or it is sewn back: the pike went with the bucket: no attacks. 1 card fewer each turn.
trait-lost-shield-emelya = Other arm lost
trait-lost-shield-emelya-help = Until the run ends or it is sewn back: no block; sleeping goes on as before. 1 card fewer each turn.
trait-lost-both-emelya = Both arms lost
trait-lost-both-emelya-help = Until the run ends or they are sewn back: the pike went with the bucket: no attacks; no block; sleeping goes on as before. 1 card and 1 energy fewer each turn.
kw-Armless = Armless
kw-Armless-help = This card can be played with no arms left.
fx-times = { $body } { $k } times.
rarity-Common = Common
rarity-Uncommon = Uncommon
rarity-Rare = Rare

# discarding and burning cards
fx-discard = Discard { $n ->
    [one] { $n } card
   *[other] { $n } cards
} of your choice.
fx-discardhand = Discard your hand.
fx-exhaustcards = Exhaust { $n ->
    [one] { $n } random card
   *[other] { $n } random cards
} in your hand.
fx-recall = Return { $n ->
    [one] { $n } random card
   *[other] { $n } random cards
} from your discard pile to your hand.
trigger-Discarded = Whenever you discard a card: { $fx }
trigger-Exhausted = Whenever a card of yours is exhausted: { $fx }
trigger-OnDiscard = Discard: { $fx }
amount-discarded = cards discarded
amount-discarded-turn = cards discarded this turn
amount-hand = cards in hand
amount-exhausted = cards exhausted this fight
kw-Discard = Discard
kw-Discard-help = A card's "Discard:" line fires when a card or a power throws it from your hand; the end of a turn does not count. You choose what to discard, and the card waits for your choice. Powers that wait for a discard fire too.

herb-nettle = Nettle
herb-henbane = Henbane
herb-valerian = Valerian
herb-st_john = St John's Wort
herb-nettle-help = In a brew: Bleed on enemies and Thorns. Spilt: 1 Bleed to all enemies.
herb-henbane-help = In a brew: Fear and Rot on enemies. Spilt: 5% Fear to all enemies.
herb-valerian-help = In a brew: Weak on enemies and Block for allies. Spilt: 1 Weak to a random enemy.
herb-st_john-help = In a brew: Strength and healing. Spilt: 1 temporary Strength.
herb-nettle-spill = 1 Bleed to all enemies
herb-henbane-spill = 5% Fear to all enemies
herb-valerian-spill = 1 Weak to a random enemy
herb-st_john-spill = +1 Strength this turn

status-Gloom = Gathering Dark
status-Gloom-help = The fight drags on: from round 9 (13 against an elite, 17 against a boss) the dark thickens every round. Each stack: 10% less Block and healing (up to 60%) and 1 HP lost at the end of the turn, through Block.

mech-Valor-name = Valour
mech-Valor = The Bogatyr gathers Valour by taking blows: 1 for every 5 damage, even taken on Block. His cards spend it to strike harder or shield the party.
mech-Curse-name = Hex and Curses
mech-Curse = The Vorozheya lays Hex (the next blows on the target deal +3, from any hero) and curses on her foes. Cards with Echo swallow them to pay out in damage or healing; many of her cards cost her own health.
mech-Runes-name = Runes
mech-Runes = The Volhv carves runes on himself (up to five) or an ally (up to three). Each fires at the end of every turn while it stands. One too many pushes out the oldest, which fires one last time. Invoke fires them all at once.
mech-Moon-name = Moon
mech-Moon = As a man the Volkolak gains 1 Moon for every attack he plays; at the fifth he turns into the beast. The Moon gathered becomes Lunar Fury: +1 Strength for every two, while he stays the beast, so waiting for the full moon pays better than turning early. The beast strikes 50% harder and heals 3 for every foe it finishes, but loses 2 health to hunger at the start of each turn and cannot give itself Block with its own cards. On the turn every card in hand flips to its beast side; Shift cards change form at will.
mech-Brew-name = Cauldron
mech-Brew = A herb does a little at once and goes into the Cauldron, up to four. Brew turns the herbs into a potion by recipe. Fern flower stands in for any herb and doubles the brew. One herb too many spills the oldest, which works weakly.
mech-Masks-name = Masks
mech-Masks = The Skomoroh wears one mask at a time. Bear: attacks deal +3 damage. Goat: every card played gives a random ally 2 Block. Mara: every blow gives the foe 7.5% Fear, up to 75%; a frightened foe may miss its attack.
mech-Sloth-name = Drowse
mech-Sloth = Every Energy left unspent in a turn gives Yemelya 1 Drowse. His cards spend Drowse on blows and Block, and those with a blue cost are paid for with it. None of his cards strike a chosen target: only a random foe, or all of them.

side-Human = As a man
side-Beast = As the beast
fx-pikewill = The Pike.
kw-PikeWill = The Pike
kw-PikeWill-help = The pike plays the top card of your draw pile by itself: for nothing, and at whoever it falls on. What cannot be played goes to the discard pile as it is.
kw-DrowseCost = Paid in Drowse
kw-DrowseCost-help = A cost on blue is paid not in Energy but in Drowse.
fx-damage-x = Deal damage: { $n }.
fx-damage-x-AllFoes = Deal damage to all enemies: { $n }.
fx-damage-x-RandomFoe = Deal damage to a random enemy: { $n }.
fx-damage-x-WeakestFoe = Deal damage to the weakest enemy: { $n }.
fx-block-x = Gain Block: { $n }.
fx-block-x-Target = Give an ally Block: { $n }.
fx-block-x-AllFriends = All allies gain Block: { $n }.
fx-block-x-OtherFriends = Other allies gain Block: { $n }.
fx-apply-ally = An ally gains { $n } { $s }.
fx-apply-ally-Me = Gain { $n } { $s }.
fx-apply-ally-AllFriends = All allies gain { $n } { $s }.
fx-apply-ally-OtherFriends = Other allies gain { $n } { $s }.
fx-apply-x = Apply { $s }: { $n }.
fx-apply-x-Me = Gain { $s }: { $n }.
fx-apply-x-AllFoes = Apply { $s } to all enemies: { $n }.
fx-apply-x-RandomFoe = Apply { $s } to a random enemy: { $n }.
fx-apply-ally-x = An ally gains { $s }: { $n }.
fx-apply-double = Double the target's { $s }.
fx-apply-double-AllFoes = Double every enemy's { $s }.
fx-consume-Me = Spend all your { $s }.
fx-draw-x = Draw cards: { $n }.
fx-draw-discarded = Draw as many cards as you discarded.
fx-energy-x = Gain Energy: { $n }.
fx-heal-OtherFriends = Heal other allies for { $n } HP.
fx-heal-x = Heal HP: { $n }.
fx-heal-x-Target = Heal an ally HP: { $n }.
fx-heal-x-AllFriends = Heal all allies HP: { $n }.
fx-losehp-x = Lose HP: { $n }.
fx-repeat-discarded = For each card discarded: { $body }
amount-stat-Target = the target's { $s }
amount-stat-AllFoes = enemies' { $s }
amount-runes = { $r } runes
cond-solo = Alone
kw-Respond = Respond
kw-Respond-help = Draws the named status off the target: it is gone, and the card's next effect grows with how much was consumed ("amount consumed").
trait-lone = Lone
trait-lone-help = Left without its kind, it strikes half again as hard.
trait-changeling = Changeling
trait-changeling-help = Carries cards off your deck. Kill it and all it took comes back to your hand.
trait-furrow = Boundary
trait-furrow-help = While the Polevik lives, the first card each turn costs 1 more Energy.
trait-paper_wings = Paper Wings
trait-paper_wings-help = The first 3 attacks each round deal him half damage.
trait-black_runes = Black Runes
trait-black_runes-help = Each rune adds 3 damage to his Invocation. A blow that takes 10+ health off him cracks one.
trait-withering = Withering
trait-withering-help = While the Shade is here, heroes heal at half.
trait-blood_scent = Smells Blood
trait-blood_scent-help = Strikes half again as hard at a hero at half health or less.
trait-twin_soul = Sisters
trait-twin_soul-help = When one sister falls, the other gains 3 Strength and raises her at 40% health 2 turns later. Kill both.
trait-eclipse = Black Sun
trait-eclipse-help = With each of Chernobog's moves the sun climbs unless he took 30 damage. At step 4, eclipse: intents are hidden and he gains 4 Strength.
trait-stolen = Cards Taken
trait-stolen-help = How many cards he has carried off your decks.

trait-glutton = Insatiable
trait-glutton-help = At the end of each turn heals 3 for every card still in the heroes' hands.
trait-gale = Wind
trait-gale-help = Each of his blows deals as much more as he has Wind. Wind rises by one per hero each round; every Skill played lulls it by 1.
trait-woe = Clinging Grief
trait-woe-help = While anyone holds a Grief card, he takes half damage. Grief can be lived through for 1 energy.
trait-house_rule-1 = Forbidden: Skills
trait-house_rule-2 = Forbidden: Powers
trait-house_rule-3 = Forbidden: Attacks
trait-house_rule-help = In his house one kind of card is forbidden each round. Whoever plays one loses 4 health through Block. The rule turns each round: Powers, Attacks, Skills.
