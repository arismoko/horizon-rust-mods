# Rust mods: AppleSkin for Sunrise

This is arismoko's fork of [Jorungandr/horizon-rust-mods](https://github.com/Jorungandr/horizon-rust-mods). Its `sunrise` branch carries one mod for [Sunrise](https://playsunrise.net) (Minecraft Java 26.3), written in Rust on Sunrise's mod API:

| Mod | Folder | Original | Licence | State |
|---|---|---|---|---|
| AppleSkin | [`native/appleskin`](native/appleskin/) | [AppleSkin](https://github.com/squeek502/AppleSkin) by squeek502 | MIT (the original is under The Unlicense) | Listed |

AppleSkin shows what the food in your hand restores, your saturation and exhaustion on the HUD, and the hunger and saturation of foods in their tooltips. Its own [README](native/appleskin/README.md) has the details.

## Install

Open the Sunrise launcher's **Discover** page and search for AppleSkin. The registry lists a signed, reviewed release built for the game build you play, and the launcher builds it on your computer when you install it.

## Build

With `cargo sunrise-mod` (from the Sunrise SDK) installed:

```sh
cd native/appleskin
cargo sunrise-mod build    # compiles against the game build pinned in sunrise-mod.toml
cargo sunrise-mod run      # builds it and starts the game with it
cargo test
```

Releases are tagged `appleskin/v<version>`; the shared Sunrise release workflow builds and signs each one.

## The other folders

Everything else here (`mods/`, `tools/`, `docs/`, `evidence/` and the other folders under `native/`) is the upstream project's earlier work by Jorungandr, written for an older game and mod format. It is kept as it is, with its own notices in `NOTICE.md`; it does not build on Sunrise and nothing is released from it on this fork.
