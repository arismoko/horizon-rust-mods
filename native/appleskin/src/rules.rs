//! AppleSkin's rules as plain Rust: the config, what a food restores, how many hearts that heals,
//! where the HUD icons go and what the tooltip shows. Nothing here touches the game, so the unit
//! tests cover it.

use std::collections::HashMap;

use mod_api::Tag;

/// `config/appleskin-client.toml` as written when it is missing.
pub const DEFAULT_CLIENT: &str = "[client]\n\t#If true, shows the hunger and saturation values of food in its tooltip while holding SHIFT\n\tshowFoodValuesInTooltip = true\n\t#If true, shows the hunger and saturation values of food in its tooltip automatically (without needing to hold SHIFT)\n\tshowFoodValuesInTooltipAlways = true\n\t#If true, shows your current saturation level overlayed on the hunger bar\n\tshowSaturationHudOverlay = true\n\t#If true, shows the hunger (and saturation if showSaturationHudOverlay is true) that would be restored by food you are currently holding\n\tshowFoodValuesHudOverlay = true\n\t#If true, enables the hunger/saturation/health overlays for food in your off-hand\n\tshowFoodValuesHudOverlayWhenOffhand = true\n\t#If true, shows your food exhaustion as a progress bar behind the hunger bar\n\tshowFoodExhaustionHudUnderlay = true\n\t#If true, shows estimated health restored by food on the health bar\n\tshowFoodHealthHudOverlay = true\n\t#If true, health/hunger overlay will shake to match Minecraft's icon animations\n\tshowVanillaAnimationsOverlay = true\n\t#Alpha value of the flashing icons at their most visible point (1.0 = fully opaque, 0.0 = fully transparent)\n\t#Range: 0.0 ~ 1.0\n\tmaxHudOverlayFlashAlpha = 0.65\n";

/// The client options (`ModConfig`).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub tooltip_shift: bool,
    pub tooltip_always: bool,
    pub saturation_overlay: bool,
    pub food_overlay: bool,
    pub food_overlay_offhand: bool,
    pub exhaustion_underlay: bool,
    pub health_overlay: bool,
    pub animations: bool,
    pub max_flash_alpha: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tooltip_shift: true,
            tooltip_always: true,
            saturation_overlay: true,
            food_overlay: true,
            food_overlay_offhand: true,
            exhaustion_underlay: true,
            health_overlay: true,
            animations: true,
            max_flash_alpha: 0.65,
        }
    }
}

/// Every `key = value` of the file, the value as written, the last of a key winning.
fn values(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.split('#').next().unwrap_or("").trim();
            map.insert(key.trim().to_owned(), value.to_owned());
        }
    }
    map
}

impl Settings {
    /// Reads the file's text. A missing, unreadable or out-of-range value keeps its default.
    pub fn parse(text: &str) -> Self {
        let map = values(text);
        let flag = |key: &str, default: bool| match map.get(key).map(String::as_str) {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        let base = Self::default();
        Self {
            tooltip_shift: flag("showFoodValuesInTooltip", base.tooltip_shift),
            tooltip_always: flag("showFoodValuesInTooltipAlways", base.tooltip_always),
            saturation_overlay: flag("showSaturationHudOverlay", base.saturation_overlay),
            food_overlay: flag("showFoodValuesHudOverlay", base.food_overlay),
            food_overlay_offhand: flag(
                "showFoodValuesHudOverlayWhenOffhand",
                base.food_overlay_offhand,
            ),
            exhaustion_underlay: flag("showFoodExhaustionHudUnderlay", base.exhaustion_underlay),
            health_overlay: flag("showFoodHealthHudOverlay", base.health_overlay),
            animations: flag("showVanillaAnimationsOverlay", base.animations),
            max_flash_alpha: map
                .get("maxHudOverlayFlashAlpha")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| (0.0..=1.0).contains(v))
                .unwrap_or(base.max_flash_alpha),
        }
    }

    /// `TooltipOverlayHandler.shouldShowTooltip`, past the stack's own checks.
    pub fn tooltip_wanted(&self, shift_down: bool) -> bool {
        (self.tooltip_shift && shift_down) || self.tooltip_always
    }
}

// ---- food -------------------------------------------------------------------------------------

/// `FoodProperties`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Food {
    pub nutrition: i32,
    pub saturation: f32,
    pub can_always_eat: bool,
}

/// One status effect of a consume effect: `id`, ticks, and amplifier from 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub id: String,
    pub duration: i32,
    pub amplifier: i32,
}

fn int_of(tag: &Tag) -> Option<i32> {
    Some(match tag {
        Tag::Byte(v) => i32::from(*v),
        Tag::Short(v) => i32::from(*v),
        Tag::Int(v) => *v,
        Tag::Long(v) => *v as i32,
        Tag::Float(bits) => f32::from_bits(*bits) as i32,
        Tag::Double(bits) => f64::from_bits(*bits) as i32,
        _ => return None,
    })
}

fn float_of(tag: &Tag) -> Option<f32> {
    Some(match tag {
        Tag::Byte(v) => f32::from(*v),
        Tag::Short(v) => f32::from(*v),
        Tag::Int(v) => *v as f32,
        Tag::Long(v) => *v as f32,
        Tag::Float(bits) => f32::from_bits(*bits),
        Tag::Double(bits) => f64::from_bits(*bits) as f32,
        _ => return None,
    })
}

impl Food {
    /// Decodes a `minecraft:food` component. A field it can't read keeps vanilla's default.
    pub fn from_tag(tag: &Tag) -> Self {
        Self {
            nutrition: tag.get("nutrition").and_then(int_of).unwrap_or(0),
            saturation: tag.get("saturation").and_then(float_of).unwrap_or(0.0),
            can_always_eat: tag.get("can_always_eat").and_then(int_of).unwrap_or(0) != 0,
        }
    }
}

/// The status effects the `apply_effects` consume effects of a `minecraft:consumable`
/// component give, in order, with the consume effect each belongs to.
pub fn consume_effects(tag: &Tag) -> Vec<Vec<Effect>> {
    let Some(Tag::List(entries)) = tag.get("on_consume_effects") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| matches!(entry.get("type"), Some(Tag::String(t)) if t.to_string_lossy() == "minecraft:apply_effects"))
        .map(|entry| {
            let Some(Tag::List(effects)) = entry.get("effects") else {
                return Vec::new();
            };
            effects
                .iter()
                .filter_map(|effect| {
                    let Some(Tag::String(id)) = effect.get("id") else {
                        return None;
                    };
                    Some(Effect {
                        id: id.to_string_lossy(),
                        duration: effect.get("duration").and_then(int_of).unwrap_or(600),
                        amplifier: effect.get("amplifier").and_then(int_of).unwrap_or(0),
                    })
                })
                .collect()
        })
        .collect()
}

/// A food as the HUD and tooltip use it (`FoodHelper.QueriedFoodResult`): its values, whether
/// it has a harmful effect (the "hunger" icons), and its Regeneration effects as
/// `(duration, amplifier)`, one per consume effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub food: Food,
    pub rotten: bool,
    pub regeneration: Vec<(i32, i32)>,
}

/// The first Regeneration of each consume effect (`FoodHelper.getEstimatedHealthIncrement`).
pub fn regeneration_of(effects: &[Vec<Effect>]) -> Vec<(i32, i32)> {
    effects
        .iter()
        .filter_map(|group| {
            group
                .iter()
                .find(|effect| effect.id == "minecraft:regeneration")
                .map(|effect| (effect.duration, effect.amplifier))
        })
        .collect()
}

const REGEN_EXHAUSTION_INCREMENT: f32 = 6.0;
const MAX_EXHAUSTION: f32 = 4.0;

/// The hearts natural regeneration heals from this state until the food level drops below 18
/// (`FoodHelper.getEstimatedHealthIncrement(int, float, float)`).
pub fn natural_regeneration(mut food: i32, mut saturation: f32, mut exhaustion: f32) -> f32 {
    if !exhaustion.is_finite() || !saturation.is_finite() {
        return 0.0;
    }
    let mut health = 0.0_f32;
    while food >= 18 {
        while exhaustion > MAX_EXHAUSTION {
            exhaustion -= MAX_EXHAUSTION;
            if saturation > 0.0 {
                saturation = (saturation - 1.0).max(0.0);
            } else {
                food -= 1;
            }
        }
        if food >= 20 && saturation > f32::MIN_POSITIVE {
            let limited = saturation.min(REGEN_EXHAUSTION_INCREMENT);
            let until_above_max = MAX_EXHAUSTION.next_up() - exhaustion;
            let iterations = ((until_above_max / limited).ceil() as i32).max(1);
            health += (limited / REGEN_EXHAUSTION_INCREMENT) * iterations as f32;
            exhaustion += limited * iterations as f32;
        } else if food >= 18 {
            health += 1.0;
            exhaustion += REGEN_EXHAUSTION_INCREMENT;
        }
    }
    health
}

/// What the player has when the estimate is asked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub food_level: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub hurt: bool,
    pub natural_regeneration: bool,
}

/// `FoodHelper.getEstimatedHealthIncrement(player, food)`: the hearts-in-halves `held` heals.
pub fn health_increment(body: &Body, held: &Held) -> f32 {
    if !body.hurt {
        return 0.0;
    }
    let level = (body.food_level + held.food.nutrition).min(20);
    let mut health = 0.0_f32;
    if level >= 18 && body.natural_regeneration {
        let saturation = (body.saturation + held.food.saturation).min(level as f32);
        health = natural_regeneration(level, saturation, body.exhaustion);
    }
    for &(duration, amplifier) in &held.regeneration {
        let period = 50_i32
            .checked_shr(u32::try_from(amplifier).unwrap_or(32))
            .unwrap_or(0)
            .max(1);
        health += (duration / period) as f32;
    }
    health
}

// ---- the HUD -----------------------------------------------------------------------------------

/// `java.util.Random`, which vanilla seeds per HUD tick so the icons shake the same way.
pub struct JavaRandom(u64);

impl JavaRandom {
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self((seed as u64 ^ 0x5_DEEC_E66D) & Self::MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = self.0.wrapping_mul(0x5_DEEC_E66D).wrapping_add(0xB) & Self::MASK;
        (self.0 >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (bound - 1) == 0 {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }
}

/// An icon's offset from its bar's anchor.
pub type Offset = (i32, i32);

/// What the icon offsets depend on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarState {
    pub max_health: f32,
    pub absorption: f32,
    pub health: f32,
    pub food_level: i32,
    pub saturation: f32,
    pub animations: bool,
}

/// The offsets of the ten food icons and the health icons, made together because the health
/// icons' draws move the food icons' (`OffsetsCache`).
#[derive(Default)]
pub struct Offsets {
    last_tick: i32,
    pub food: Vec<Offset>,
    pub health: Vec<Offset>,
}

impl Offsets {
    /// Makes the offsets of `tick` unless they are the ones kept.
    pub fn at(&mut self, tick: i32, state: &BarState) -> &Self {
        if tick != self.last_tick {
            self.generate(tick, state);
            self.last_tick = tick;
        }
        self
    }

    fn generate(&mut self, tick: i32, state: &BarState) {
        let absorption = state.absorption.ceil();
        let mut bars = ((state.max_health + absorption) / 2.0).ceil() as i32;
        if !(0..=1000).contains(&bars) {
            bars = 0;
        }
        let rows = (bars as f32 / 10.0).ceil() as i32;
        let row_height = (10 - (rows - 2)).max(3);
        let shake_food = state.animations
            && state.saturation <= 0.0
            && tick.checked_rem(state.food_level * 3 + 1) == Some(0);
        let shake_health = state.animations && f64::from(state.health).ceil() <= 4.0;
        let mut random = JavaRandom::new(i64::from(tick) * 312_871);
        self.health = vec![(0, 0); bars as usize];
        for i in (0..bars).rev() {
            let row = ((i + 1) as f32 / 10.0).ceil() as i32 - 1;
            let mut y = -(row * row_height);
            if shake_health {
                y += random.next_int(2);
            }
            self.health[i as usize] = (i % 10 * 8, y);
        }
        self.food = (0..10)
            .map(|i| {
                let mut y = 0;
                if shake_food {
                    y += random.next_int(3) - 1;
                }
                (-(i * 8) - 9, y)
            })
            .collect();
    }
}

/// The vanilla sprites the overlays borrow (`TextureHelper`), and the mod's own.
pub const ICONS: &str = "appleskin:gui/icons";
pub const OUTLINE: &str = "appleskin:gui/tooltip_hunger_outline";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FoodIcon {
    Empty,
    Half,
    Full,
}

pub fn food_sprite(rotten: bool, icon: FoodIcon) -> &'static str {
    match (rotten, icon) {
        (false, FoodIcon::Empty) => "hud/food_empty",
        (false, FoodIcon::Half) => "hud/food_half",
        (false, FoodIcon::Full) => "hud/food_full",
        (true, FoodIcon::Empty) => "hud/food_empty_hunger",
        (true, FoodIcon::Half) => "hud/food_half_hunger",
        (true, FoodIcon::Full) => "hud/food_full_hunger",
    }
}

pub fn heart_sprite(hardcore: bool, icon: FoodIcon) -> &'static str {
    match (hardcore, icon) {
        (false, FoodIcon::Empty) => "hud/heart/container",
        (false, FoodIcon::Half) => "hud/heart/half",
        (false, FoodIcon::Full) => "hud/heart/full",
        (true, FoodIcon::Empty) => "hud/heart/container_hardcore",
        (true, FoodIcon::Half) => "hud/heart/hardcore_half",
        (true, FoodIcon::Full) => "hud/heart/hardcore_full",
    }
}

/// `ColorHelper.argbFromRGBA` with white.
pub fn white(alpha: f32) -> u32 {
    argb(1.0, 1.0, 1.0, alpha)
}

pub fn argb(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    channel(a) << 24 | channel(r) << 16 | channel(g) << 8 | channel(b)
}

/// One sprite to draw: `src` and `dst` as `DrawContext::sprite` takes them.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub name: &'static str,
    pub src: [f32; 4],
    pub dst: [f32; 4],
    pub tint: u32,
}

fn icon(name: &'static str, x: i32, y: i32, tint: u32) -> Sprite {
    Sprite {
        name,
        src: [0.0, 0.0, 9.0, 9.0],
        dst: [x as f32, y as f32, 9.0, 9.0],
        tint,
    }
}

/// `HUDOverlayHandler.drawSaturationOverlay`: `level` is the saturation shown and `gained` what
/// the held food adds on top.
pub fn saturation_sprites(
    gained: f32,
    level: f32,
    right: i32,
    top: i32,
    alpha: f32,
    offsets: &[Offset],
) -> Vec<Sprite> {
    if level + gained < 0.0 {
        return Vec::new();
    }
    let modified = (level + gained).clamp(0.0, 20.0);
    let end = (modified / 2.0).ceil() as i32;
    let start = if gained != 0.0 {
        (level / 2.0).max(0.0) as i32
    } else {
        0
    };
    (start..end)
        .map(|i| {
            let (dx, dy) = offsets.get(i as usize).copied().unwrap_or((0, 0));
            let effective = modified / 2.0 - i as f32;
            let u = if effective >= 1.0 {
                27.0
            } else if effective > 0.5 {
                18.0
            } else if effective > 0.25 {
                9.0
            } else {
                0.0
            };
            Sprite {
                name: ICONS,
                src: [u, 0.0, 9.0, 9.0],
                dst: [(right + dx) as f32, (top + dy) as f32, 9.0, 9.0],
                tint: white(alpha),
            }
        })
        .collect()
}

/// `HUDOverlayHandler.drawHungerOverlay`.
pub fn hunger_sprites(
    restored: i32,
    food_level: i32,
    right: i32,
    top: i32,
    alpha: f32,
    rotten: bool,
    offsets: &[Offset],
) -> Vec<Sprite> {
    if restored <= 0 {
        return Vec::new();
    }
    let modified = (food_level + restored).clamp(0, 20);
    let start = (food_level / 2).max(0);
    let end = (modified as f32 / 2.0).ceil() as i32;
    let mut out = Vec::new();
    for i in start..end {
        let (dx, dy) = offsets.get(i as usize).copied().unwrap_or((0, 0));
        let (x, y) = (right + dx, top + dy);
        out.push(icon(
            food_sprite(rotten, FoodIcon::Empty),
            x,
            y,
            white(alpha * 0.25),
        ));
        let half = if i * 2 + 1 == modified {
            FoodIcon::Half
        } else {
            FoodIcon::Full
        };
        out.push(icon(food_sprite(rotten, half), x, y, white(alpha)));
    }
    out
}

/// `HUDOverlayHandler.drawHealthOverlay`: the hearts between `health` and `modified`.
pub fn health_sprites(
    health: f32,
    modified: f32,
    left: i32,
    top: i32,
    alpha: f32,
    hardcore: bool,
    offsets: &[Offset],
) -> Vec<Sprite> {
    if modified <= health {
        return Vec::new();
    }
    let fixed = modified.ceil() as i32;
    let start = (f64::from(health).ceil() as f32 / 2.0).max(0.0) as i32;
    let end = (modified / 2.0).ceil().max(0.0) as i32;
    let mut out = Vec::new();
    for i in start..end {
        let (dx, dy) = offsets.get(i as usize).copied().unwrap_or((0, 0));
        let (x, y) = (left + dx, top + dy);
        out.push(icon(
            heart_sprite(hardcore, FoodIcon::Empty),
            x,
            y,
            white(alpha * 0.25),
        ));
        let half = if i * 2 + 1 == fixed {
            FoodIcon::Half
        } else {
            FoodIcon::Full
        };
        out.push(icon(heart_sprite(hardcore, half), x, y, white(alpha)));
    }
    out
}

/// `HUDOverlayHandler.drawExhaustionOverlay`: the bar behind the food icons.
pub fn exhaustion_sprite(exhaustion: f32, right: i32, top: i32) -> Sprite {
    let ratio = (exhaustion / MAX_EXHAUSTION).clamp(0.0, 1.0);
    let width = (ratio * 81.0) as i32;
    Sprite {
        name: ICONS,
        src: [(81 - width) as f32, 18.0, width as f32, 9.0],
        dst: [(right - width) as f32, top as f32, width as f32, 9.0],
        tint: white(0.75),
    }
}

/// The estimated-health overlay's own conditions (`shouldShowEstimatedHealth`).
pub fn shows_estimated_health(
    enabled: bool,
    peaceful: bool,
    food_level: i32,
    poisoned_withered_or_regenerating: bool,
) -> bool {
    enabled && !peaceful && food_level < 18 && !poisoned_withered_or_regenerating
}

/// The flashing alpha the held food's icons pulse with, stepped once a client tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flash {
    unclamped: f32,
    pub alpha: f32,
    dir: i8,
}

impl Default for Flash {
    fn default() -> Self {
        Self {
            unclamped: 0.0,
            alpha: 0.0,
            dir: 1,
        }
    }
}

impl Flash {
    pub fn step(&mut self, max_alpha: f32) {
        self.unclamped += f32::from(self.dir) * 0.125;
        if self.unclamped >= 1.5 {
            self.dir = -1;
        } else if self.unclamped <= -0.5 {
            self.dir = 1;
        }
        self.alpha = self.unclamped.clamp(0.0, 1.0) * max_alpha.clamp(0.0, 1.0);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `HUDOverlayHandler.shouldRenderOverlay`: survival and adventure players only, with the HUD on.
pub fn hud_applies(hud_hidden: bool, can_hurt: bool) -> bool {
    !hud_hidden && can_hurt
}

// ---- the tooltip -------------------------------------------------------------------------------

/// A line of tooltip text: the text, its position, its scale and its colour.
pub type TextLine = (String, [f32; 2], f32, u32);

/// `FoodOutline`, the outline of a hunger icon in the tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    Negative,
    Extra,
    Normal,
    Partial,
    Missing,
}

impl Outline {
    pub fn argb(self) -> u32 {
        match self {
            Self::Negative => argb(1.0, 1.0, 1.0, 1.0),
            Self::Extra => argb(0.06, 0.32, 0.02, 1.0),
            Self::Normal => argb(0.0, 0.0, 0.0, 1.0),
            Self::Partial => argb(0.53, 0.21, 0.08, 1.0),
            Self::Missing => argb(0.62, 0.0, 0.0, 0.5),
        }
    }

    pub fn of(modified: i32, default: i32, i: i32) -> Self {
        if modified < 0 {
            Self::Negative
        } else if modified > default && default <= i {
            Self::Extra
        } else if modified > i + 1 || default == modified {
            Self::Normal
        } else if modified == i + 1 {
            Self::Partial
        } else {
            Self::Missing
        }
    }
}

/// `FoodTooltip`: how many icons each row has, or the "x N" text that stands for them.
#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    pub default: Food,
    pub modified: Food,
    pub rotten: bool,
    pub hunger_bars: i32,
    pub hunger_text: Option<String>,
    pub saturation_bars: i32,
    pub saturation_text: Option<String>,
}

impl Tooltip {
    pub fn new(default: Food, modified: Food, rotten: bool) -> Self {
        let biggest_hunger = default.nutrition.max(modified.nutrition);
        let biggest_saturation = default.saturation.max(modified.saturation);
        let mut hunger_bars = (f64::from(biggest_hunger.unsigned_abs()) / 2.0).ceil() as i32;
        let mut hunger_text = None;
        if hunger_bars > 10 {
            let sign = if biggest_hunger < 0 { -1 } else { 1 };
            hunger_text = Some(format!("x{}", sign * hunger_bars));
            hunger_bars = 1;
        }
        let mut saturation_bars = (biggest_saturation.abs() / 2.0).ceil() as i32;
        let mut saturation_text = None;
        if saturation_bars > 10 || saturation_bars == 0 {
            let sign = if biggest_saturation < 0.0 { -1 } else { 1 };
            saturation_text = Some(format!("x{}", sign * saturation_bars));
            saturation_bars = 1;
        }
        Self {
            default,
            modified,
            rotten,
            hunger_bars,
            hunger_text,
            saturation_bars,
            saturation_text,
        }
    }

    /// `shouldRenderHungerBars`.
    pub fn shown(&self) -> bool {
        self.hunger_bars > 0
    }

    /// `FoodTooltipRenderer.getHeight`: hunger row, spacing, saturation row, and the 3 extra
    /// pixels that look best.
    pub const HEIGHT: f32 = 9.0 + 1.0 + 7.0 + 3.0;

    /// `FoodTooltipRenderer.getWidth`, given the width of a string in GUI pixels.
    pub fn width(&self, text_width: &dyn Fn(&str) -> f32) -> f32 {
        let hunger =
            self.hunger_bars as f32 * 9.0 + self.hunger_text.as_deref().map_or(0.0, text_width);
        let saturation = self.saturation_bars as f32 * 7.0
            + self.saturation_text.as_deref().map_or(0.0, text_width);
        hunger.max(saturation) + 2.0
    }

    /// `FoodTooltipRenderer.extractImage`: what to draw with the top left at `(x, y)`.
    pub fn draw(&self, x: f32, y: f32) -> (Vec<Sprite>, Vec<TextLine>) {
        let mut sprites = Vec::new();
        let mut texts = Vec::new();
        let default = self.default.nutrition;
        let modified = self.modified.nutrition;
        let mut offset_x = x + (self.hunger_bars - 1) as f32 * 9.0;
        let tinted = |name, at_x: f32, tint| Sprite {
            name,
            src: [0.0, 0.0, 9.0, 9.0],
            dst: [at_x, y, 9.0, 9.0],
            tint,
        };
        for i in (0..self.hunger_bars * 2).step_by(2) {
            sprites.push(tinted(
                food_sprite(false, FoodIcon::Empty),
                offset_x,
                white(1.0),
            ));
            let outline = Outline::of(modified, default, i);
            if outline != Outline::Normal {
                sprites.push(tinted(OUTLINE, offset_x, outline.argb()));
            }
            let default_icon = if default - 1 == i {
                FoodIcon::Half
            } else {
                FoodIcon::Full
            };
            sprites.push(tinted(
                food_sprite(self.rotten, default_icon),
                offset_x,
                white(0.25),
            ));
            if modified > i {
                let icon = if modified - 1 == i {
                    FoodIcon::Half
                } else {
                    FoodIcon::Full
                };
                sprites.push(tinted(food_sprite(self.rotten, icon), offset_x, white(1.0)));
            }
            offset_x -= 9.0;
        }
        if let Some(text) = &self.hunger_text {
            offset_x += 18.0;
            texts.push((text.clone(), [offset_x + 1.5, y + 1.5], 0.75, 0xFFAA_AAAA));
        }
        let row_y = y + 10.0;
        let modified_saturation = self.modified.saturation;
        let absolute = modified_saturation.abs();
        let mut offset_x = x + (self.saturation_bars - 1) as f32 * 7.0;
        for i in (0..self.saturation_bars * 2).step_by(2) {
            let effective = (absolute - i as f32) / 2.0;
            let tint = if absolute <= i as f32 {
                white(0.5)
            } else {
                white(1.0)
            };
            let u = if effective >= 1.0 {
                21.0
            } else if effective > 0.5 {
                14.0
            } else if effective > 0.25 {
                7.0
            } else if effective > 0.0 {
                0.0
            } else {
                28.0
            };
            let v = if modified_saturation >= 0.0 {
                27.0
            } else {
                34.0
            };
            sprites.push(Sprite {
                name: ICONS,
                src: [u, v, 7.0, 7.0],
                dst: [offset_x, row_y, 7.0, 7.0],
                tint,
            });
            offset_x -= 7.0;
        }
        if let Some(text) = &self.saturation_text {
            offset_x += 14.0;
            texts.push((
                text.clone(),
                [offset_x + 1.5, row_y + 0.75],
                0.75,
                0xFFAA_AAAA,
            ));
        }
        (sprites, texts)
    }
}

// ---- the debug line -----------------------------------------------------------------------------

/// Java's `DecimalFormat("#.##")`: up to two decimals, none when they are zero.
pub fn at_most_two(value: f32) -> String {
    let rounded = (f64::from(value) * 100.0).round_ties_even() / 100.0;
    let text = format!("{rounded:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// `DebugInfoHandler.FoodStatsDebugEntry`.
pub fn debug_line(food_level: i32, saturation: f32, exhaustion: f32) -> String {
    format!(
        "hunger: {food_level}, sat: {}, exh: {:.2}/{}",
        at_most_two(saturation),
        (f64::from(exhaustion) * 100.0).round_ties_even() / 100.0,
        at_most_two(MAX_EXHAUSTION)
    )
}

// ---- the sync payloads --------------------------------------------------------------------------

/// The server sends exhaustion when it moved this much (`SyncHandler`).
pub const EXHAUSTION_STEP: f32 = 0.01;

pub const CHANNEL_SATURATION: &str = "appleskin:saturation";
pub const CHANNEL_EXHAUSTION: &str = "appleskin:exhaustion";
pub const CHANNEL_NATURAL_REGENERATION: &str = "appleskin:natural_regeneration";

pub fn encode_float(value: f32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

pub fn decode_float(bytes: &[u8]) -> Option<f32> {
    Some(f32::from_be_bytes(bytes.try_into().ok()?))
}

pub fn encode_flag(value: bool) -> Vec<u8> {
    vec![u8::from(value)]
}

pub fn decode_flag(bytes: &[u8]) -> Option<bool> {
    match bytes {
        [0] => Some(false),
        [1] => Some(true),
        _ => None,
    }
}

/// Whether a player's exhaustion has moved far enough from the last one sent.
pub fn exhaustion_changed(last: Option<f32>, now: f32) -> bool {
    last.is_none_or(|last| (last - now).abs() >= EXHAUSTION_STEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compound(entries: Vec<(&str, Tag)>) -> Tag {
        Tag::compound(entries)
    }

    fn float(v: f32) -> Tag {
        Tag::Float(v.to_bits())
    }

    #[test]
    fn config_defaults_match_the_file_and_bad_values_fall_back() {
        assert_eq!(Settings::parse(DEFAULT_CLIENT), Settings::default());
        let s = Settings::parse(
            "showFoodValuesInTooltipAlways = false # no\nmaxHudOverlayFlashAlpha = 1.5\nshowSaturationHudOverlay = maybe\n",
        );
        assert!(!s.tooltip_always);
        assert_eq!(s.max_flash_alpha, 0.65);
        assert!(s.saturation_overlay);
        assert!(Settings::parse("maxHudOverlayFlashAlpha = 0.0\n").max_flash_alpha == 0.0);
    }

    #[test]
    fn tooltip_is_wanted_with_shift_or_always() {
        let mut s = Settings::default();
        assert!(s.tooltip_wanted(false));
        s.tooltip_always = false;
        assert!(!s.tooltip_wanted(false));
        assert!(s.tooltip_wanted(true));
        s.tooltip_shift = false;
        assert!(!s.tooltip_wanted(true));
    }

    #[test]
    fn food_reads_every_number_form_of_the_item_report() {
        let tag = compound(vec![
            ("nutrition", Tag::Int(6)),
            ("saturation", Tag::Double(9.6_f64.to_bits())),
            ("can_always_eat", Tag::Byte(1)),
        ]);
        let food = Food::from_tag(&tag);
        assert_eq!(food.nutrition, 6);
        assert!((food.saturation - 9.6).abs() < 1e-6);
        assert!(food.can_always_eat);
        assert_eq!(Food::from_tag(&compound(vec![])), Food::default());
        let patch = compound(vec![("nutrition", Tag::Int(4)), ("saturation", float(2.5))]);
        assert_eq!(Food::from_tag(&patch).saturation, 2.5);
    }

    #[test]
    fn consume_effects_collect_apply_effects_only() {
        let regen = compound(vec![
            ("id", Tag::String("minecraft:regeneration".into())),
            ("duration", Tag::Int(100)),
            ("amplifier", Tag::Byte(1)),
        ]);
        let poison = compound(vec![("id", Tag::String("minecraft:poison".into()))]);
        let tag = compound(vec![(
            "on_consume_effects",
            Tag::List(vec![
                compound(vec![
                    ("type", Tag::String("minecraft:apply_effects".into())),
                    ("effects", Tag::List(vec![poison, regen])),
                ]),
                compound(vec![(
                    "type",
                    Tag::String("minecraft:clear_all_effects".into()),
                )]),
            ]),
        )]);
        let effects = consume_effects(&tag);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0][0].duration, 600);
        assert_eq!(regeneration_of(&effects), vec![(100, 1)]);
        assert!(consume_effects(&compound(vec![])).is_empty());
    }

    #[test]
    fn natural_regeneration_matches_a_hand_run() {
        // Food 20 with 0 saturation: slow regen only. Each heal costs 6 exhaustion; the loop
        // stops once food falls under 18.
        // heal (exh 6), reduce: 6>4 -> 2 (food 19); heal (exh 8): 8>4 -> 4 (food 18), 4 !> 4;
        // heal (exh 10): 10>4 -> 6>4 -> 2 (food 17, 16).
        assert_eq!(natural_regeneration(20, 0.0, 0.0), 3.0);
        assert_eq!(natural_regeneration(17, 5.0, 0.0), 0.0);
        assert_eq!(natural_regeneration(20, f32::NAN, 0.0), 0.0);
        // Full saturation 20: fast regen gives saturation/6 per tick of exhaustion.
        let fast = natural_regeneration(20, 6.0, 0.0);
        assert!(fast > 6.0);
    }

    #[test]
    fn health_increment_needs_a_hurt_player_and_adds_regeneration() {
        let held = Held {
            food: Food {
                nutrition: 4,
                saturation: 2.4,
                can_always_eat: false,
            },
            rotten: false,
            regeneration: vec![(100, 0), (900, 1)],
        };
        let mut body = Body {
            food_level: 10,
            saturation: 0.0,
            exhaustion: 0.0,
            hurt: false,
            natural_regeneration: true,
        };
        assert_eq!(health_increment(&body, &held), 0.0);
        body.hurt = true;
        // 14 food: no natural regeneration. 100/50 + 900/25.
        assert_eq!(health_increment(&body, &held), 2.0 + 36.0);
        body.food_level = 16;
        let with_regen = health_increment(&body, &held);
        assert!(with_regen > 38.0);
        body.natural_regeneration = false;
        assert_eq!(health_increment(&body, &held), 38.0);
    }

    #[test]
    fn java_random_follows_the_documented_sequence() {
        // java.util.Random(42).nextInt(10) gives 0, then 3, 8 (the first three draws).
        let mut random = JavaRandom::new(42);
        assert_eq!(random.next_int(10), 0);
        assert_eq!(random.next_int(10), 3);
        assert_eq!(random.next_int(10), 8);
        // A power-of-two bound uses the high bits: Random(42).nextInt(2).
        assert_eq!(JavaRandom::new(42).next_int(2), 1);
    }

    fn calm(health: f32, max: f32) -> BarState {
        BarState {
            max_health: max,
            absorption: 0.0,
            health,
            food_level: 20,
            saturation: 5.0,
            animations: true,
        }
    }

    #[test]
    fn offsets_put_food_right_to_left_and_hearts_in_rows() {
        let mut offsets = Offsets::default();
        let o = offsets.at(7, &calm(20.0, 20.0));
        assert_eq!(o.food[0], (-9, 0));
        assert_eq!(o.food[9], (-81, 0));
        assert_eq!(o.health.len(), 10);
        assert_eq!(o.health[9], (72, 0));
        // 30 health: two rows 10 apart; 50 health: three rows 9 apart.
        let mut offsets = Offsets::default();
        let o = offsets.at(7, &calm(30.0, 30.0));
        assert_eq!(o.health.len(), 15);
        assert_eq!(o.health[10], (0, -10));
        let mut offsets = Offsets::default();
        let o = offsets.at(7, &calm(50.0, 50.0));
        assert_eq!(o.health[20], (0, -18));
        // Huge health disables the hearts.
        let mut offsets = Offsets::default();
        assert!(offsets.at(7, &calm(20.0, 1e9)).health.is_empty());
    }

    #[test]
    fn low_health_and_empty_saturation_shake_the_icons() {
        let mut offsets = Offsets::default();
        let mut state = calm(4.0, 20.0);
        state.saturation = 0.0;
        // food 20: shake when tick % 61 == 0.
        let o = offsets.at(61, &state);
        let mut random = JavaRandom::new(61 * 312_871);
        let expected: Vec<i32> = (0..10).rev().map(|_| random.next_int(2)).collect();
        for (i, want) in (0..10).rev().zip(&expected) {
            assert_eq!(o.health[i].1, *want);
        }
        assert!(o.food.iter().all(|&(_, y)| (-1..=1).contains(&y)));
        // animations off: no shaking at all.
        state.animations = false;
        let mut offsets = Offsets::default();
        let o = offsets.at(61, &state);
        assert!(o.health.iter().all(|&(_, y)| y == 0));
    }

    #[test]
    fn hunger_overlay_covers_the_restored_icons() {
        let offsets: Vec<Offset> = (0..10).map(|i| (-(i * 8) - 9, 0)).collect();
        // Food 7, +3: icons 3 and 4.
        let sprites = hunger_sprites(3, 7, 100, 200, 1.0, false, &offsets);
        assert_eq!(sprites.len(), 4);
        assert_eq!(sprites[0].name, "hud/food_empty");
        assert_eq!(sprites[0].dst[0], (100 - 3 * 8 - 9) as f32);
        assert_eq!(sprites[1].name, "hud/food_full");
        // 7 + 3 = 10 ends on a full icon; 7 + 2 = 9 on a half one.
        let half = hunger_sprites(2, 7, 100, 200, 1.0, true, &offsets);
        assert_eq!(half.last().map(|s| s.name), Some("hud/food_half_hunger"));
        assert!(hunger_sprites(0, 7, 0, 0, 1.0, false, &offsets).is_empty());
    }

    #[test]
    fn saturation_overlay_picks_the_icon_by_the_bar_share() {
        let offsets: Vec<Offset> = (0..10).map(|i| (-(i * 8) - 9, 0)).collect();
        let sprites = saturation_sprites(0.0, 5.0, 0, 0, 1.0, &offsets);
        assert_eq!(sprites.len(), 3);
        assert_eq!(sprites[0].src[0], 27.0);
        assert_eq!(sprites[1].src[0], 27.0);
        assert_eq!(sprites[2].src[0], 9.0);
        // The gained part starts at the current tail.
        let gained = saturation_sprites(3.0, 5.0, 0, 0, 0.5, &offsets);
        assert_eq!(gained.len(), 2);
        assert_eq!(gained[0].dst[0], offsets[2].0 as f32);
        assert!(saturation_sprites(-6.0, 5.0, 0, 0, 1.0, &offsets).is_empty());
    }

    #[test]
    fn health_overlay_draws_the_gained_hearts_with_a_half_at_the_end() {
        let offsets: Vec<Offset> = (0..10).map(|i| (i * 8, 0)).collect();
        let sprites = health_sprites(8.0, 10.0, 10, 20, 1.0, false, &offsets);
        // heart 4 only: a container and a full heart.
        assert_eq!(sprites.len(), 2);
        assert_eq!(sprites[0].name, "hud/heart/container");
        assert_eq!(sprites[1].name, "hud/heart/full");
        let half = health_sprites(8.0, 9.0, 0, 0, 1.0, true, &offsets);
        assert_eq!(half.last().map(|s| s.name), Some("hud/heart/hardcore_half"));
        assert!(health_sprites(9.0, 9.0, 0, 0, 1.0, false, &offsets).is_empty());
    }

    #[test]
    fn exhaustion_bar_grows_from_the_right() {
        let s = exhaustion_sprite(2.0, 100, 50);
        assert_eq!(s.dst, [100.0 - 40.0, 50.0, 40.0, 9.0]);
        assert_eq!(s.src, [41.0, 18.0, 40.0, 9.0]);
        assert_eq!(exhaustion_sprite(9.0, 100, 50).dst[2], 81.0);
        assert_eq!(exhaustion_sprite(-1.0, 100, 50).dst[2], 0.0);
    }

    #[test]
    fn flash_pulses_between_zero_and_the_maximum() {
        let mut flash = Flash::default();
        let mut peak = 0.0_f32;
        for _ in 0..40 {
            flash.step(0.65);
            peak = peak.max(flash.alpha);
            assert!((0.0..=0.65).contains(&flash.alpha));
        }
        assert_eq!(peak, 0.65);
        flash.reset();
        assert_eq!(flash, Flash::default());
    }

    #[test]
    fn estimated_health_needs_a_quiet_hungry_player() {
        assert!(shows_estimated_health(true, false, 17, false));
        assert!(!shows_estimated_health(true, false, 18, false));
        assert!(!shows_estimated_health(true, true, 10, false));
        assert!(!shows_estimated_health(true, false, 10, true));
        assert!(!shows_estimated_health(false, false, 10, false));
        assert!(hud_applies(false, true));
        assert!(!hud_applies(true, true));
    }

    #[test]
    fn outline_follows_the_modified_value() {
        assert_eq!(Outline::of(-2, 4, 0), Outline::Negative);
        assert_eq!(Outline::of(8, 4, 4), Outline::Extra);
        assert_eq!(Outline::of(4, 4, 0), Outline::Normal);
        assert_eq!(Outline::of(5, 8, 4), Outline::Partial);
        assert_eq!(Outline::of(2, 8, 4), Outline::Missing);
        assert_eq!(Outline::Normal.argb(), 0xFF00_0000);
        assert_eq!(Outline::Missing.argb(), 0x809E_0000);
    }

    #[test]
    fn tooltip_counts_icons_and_swaps_many_for_text() {
        let apple = Food {
            nutrition: 4,
            saturation: 2.4,
            can_always_eat: false,
        };
        let t = Tooltip::new(apple, apple, false);
        assert_eq!((t.hunger_bars, t.saturation_bars), (2, 2));
        assert!(t.hunger_text.is_none() && t.saturation_text.is_none());
        let width = |s: &str| s.len() as f32 * 6.0;
        assert_eq!(t.width(&width), 2.0 * 9.0 + 2.0);
        let (sprites, texts) = t.draw(10.0, 20.0);
        assert!(texts.is_empty());
        // Two hunger icons (empty, default, modified: 3 each) and two saturation icons.
        assert_eq!(sprites.len(), 3 * 2 + 2);
        assert_eq!(sprites[0].dst[0], 19.0);
        let big = Food {
            nutrition: 30,
            saturation: 0.0,
            can_always_eat: false,
        };
        let t = Tooltip::new(big, big, false);
        assert_eq!(t.hunger_bars, 1);
        assert_eq!(t.hunger_text.as_deref(), Some("x15"));
        assert_eq!(t.saturation_text.as_deref(), Some("x0"));
        assert_eq!(t.saturation_bars, 1);
        let (_, texts) = t.draw(0.0, 0.0);
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].2, 0.75);
        assert_eq!(Tooltip::HEIGHT, 20.0);
    }

    #[test]
    fn tooltip_marks_negative_saturation_with_the_other_row() {
        let rotten = Food {
            nutrition: 4,
            saturation: -3.0,
            can_always_eat: true,
        };
        let t = Tooltip::new(rotten, rotten, true);
        let (sprites, _) = t.draw(0.0, 0.0);
        let saturation: Vec<&Sprite> = sprites.iter().filter(|s| s.src[3] == 7.0).collect();
        // The biggest saturation is the larger of 0.0 default... both -3: 2 bars, v = 34.
        assert!(saturation.iter().all(|s| s.src[1] == 34.0));
    }

    #[test]
    fn debug_line_formats_like_decimal_format() {
        assert_eq!(debug_line(20, 5.0, 0.0), "hunger: 20, sat: 5, exh: 0.00/4");
        assert_eq!(
            debug_line(14, 2.375, 1.234),
            "hunger: 14, sat: 2.38, exh: 1.23/4"
        );
        assert_eq!(at_most_two(0.0), "0");
        assert_eq!(at_most_two(-0.001), "0");
    }

    #[test]
    fn payload_floats_and_flags_round_trip() {
        assert_eq!(decode_float(&encode_float(3.25)), Some(3.25));
        assert_eq!(decode_float(&[1, 2]), None);
        assert_eq!(decode_flag(&encode_flag(true)), Some(true));
        assert_eq!(decode_flag(&[2]), None);
        assert!(exhaustion_changed(None, 0.0));
        assert!(!exhaustion_changed(Some(1.0), 1.005));
        assert!(exhaustion_changed(Some(1.0), 1.02));
    }
}
