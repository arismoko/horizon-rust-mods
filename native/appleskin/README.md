# AppleSkin for Sunrise

Food values on the HUD and in tooltips: what the food in your hand will restore, your saturation and exhaustion, and the hunger and saturation of every food.

A port of [AppleSkin](https://github.com/squeek502/AppleSkin) by squeek502 to [Sunrise](https://playsunrise.net) (Minecraft Java 26.3). The original is under The Unlicense; the port is MIT (see `LICENSE` and `NOTICE`). One native mod on Sunrise's mod API.

## What it does

- **Held food on the HUD.** While you hold a food you could eat (main hand, then off hand), the hunger icons it would fill flash over the hunger bar (green-tinted "rotten" icons when it has a harmful effect), and so does the saturation it adds.
- **Estimated health.** Hearts the food would heal, counting natural regeneration and the food's Regeneration effect, flash over the health bar when you are below 18 food, not in Peaceful, and not poisoned, withered or regenerating.
- **Saturation overlay.** Your saturation is drawn over the hunger icons.
- **Exhaustion underlay.** A bar behind the hunger icons fills as exhaustion builds towards the next food point.
- **Tooltips.** A food's tooltip gets a row of hunger icons and a row of saturation icons (a number once there are more than ten), always or while holding Shift.
- **Vanilla shaking.** The overlays shake with the icons when you are hungry with no saturation, or at low health.
- **Debug screen.** A `hunger, sat, exh` line.
- **Sync.** The server sends each client its exact saturation and exhaustion, and whether natural regeneration is on.
- `config/appleskin-client.toml`, reloaded live: the same options as the original.

## Differences from the original

- The HUD overlays sit at vanilla's bar height, 39 pixels above the bottom of the screen, whatever the bars above them hold.
- Hearts are always drawn as normal hearts; the hardcore ones are not used.
- The overlays hide while riding any living mount, not only those with a health bar.
- The debug line is in the position group of the debug screen, not a group of its own.
- The original's events for other mods and its in-game config screen are not part of this version.
