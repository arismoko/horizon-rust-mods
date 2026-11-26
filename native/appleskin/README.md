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
- The original's options on the mod's page in Options > Mods, applied as you change them.
- **Events for other mods.** `appleskin::events::FoodValuesEvent` (`appleskin:food_values`, client) lets a listener change the hunger and saturation the HUD shows for the held food; `HudOverlayEvent` (`appleskin:hud_overlay`, client, cancellable) lets a listener hide the hunger, saturation, exhaustion or health overlay. Both are posted once per client tick. Subscribe with `#[subscribe]` in a mod that lists AppleSkin under `[dependencies]`.

## Differences from the original

- There is no tooltip event (`TooltipOverlayEvent`): the tooltip rows always show the food's own values, and `FoodValuesEvent` changes only the HUD overlays.
