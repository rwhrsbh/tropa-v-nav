# Open work

Things agreed and not done yet. Take one, and strike it out here when it is in.

## Gorynych as one body with three heads

Gorynych (encounter `a4_gorynych`, act 4 elite) is three foes in the rules: `gorynych_fire`,
`gorynych_poison` and `gorynych_thunder`, each with its own health and moves. A head that is
killed grows back after two rounds with 35 HP while another head still lives
(`crates/core/src/combat.rs`, end of round, flag `gorynych_head`), so all three must fall close
together.

On screen it is wrong: each head's picture is the whole three-headed dragon, so three identical
dragons stand side by side and a killed head falls as a whole dead dragon.

To do:

1. Paint in Flow one body with three empty necks ending in bloody stumps, and the three heads
   apart (fire, poison, thunder), each with a strike and a hurt pose.
2. In the battle screen draw the shared body once and each head unit on its own neck; a dead
   head leaves its stump, and `Event::Summoned` (sent when it regrows) puts it back.
3. When all three are dead the body falls as `gorynych_dead`.

## Fight poses for the new foes

About forty foes lack some of `_strike`, `_hurt`, `_guard`, `_cast`.
`python tools/flow_pose_cmds.py <id>` prints the Flow commands for the missing ones; take them
in with `tools/pose_in.py`. Check every pose by eye: extra arms and doubled weapons happen.

## Rig parts for the rest of the new wave

pugach, tugarin, vedmak, wolf_herder, spinner, upyr_knyaz, chernobog, zhlya, vikhor:
`tools/flow_rig_cmds.py`, then `tools/rig_in.py` (see art_pipeline.md).
