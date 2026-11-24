//! AppleSkin for Sunrise: the HUD shows what the held food restores, the saturation and the
//! exhaustion behind the hunger bar, and food tooltips show their hunger and saturation.
//!
//! | Moment | Where | What it does |
//! |---|---|---|
//! | `register_gui_layers` | [`layers`] | health, hunger, saturation and exhaustion layers beside vanilla's |
//! | `register` | this module | the sync payloads |
//! | `render_tooltip_gather_components` | this module | a [`FoodTooltip`] after the lines of every food's tooltip |
//! | `register_client_tooltip_component_factories` | [`ClientFoodTooltip`] | the hunger and saturation rows |
//! | `client_tick_post` | this module | the flashing alpha |
//! | `player_tick_post`, `player_logged_in`, `server_tick` | [`sync`] | saturation, exhaustion and natural regeneration sent to clients |
//! | `register_debug_entries` | this module | the `hunger, sat, exh` line of the debug screen |
//! | `register` | this module | the client options, on the mod's page in Options > Mods |

pub mod rules;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use mod_api::{
    ClientHost, ClientPlayerLoggingInEvent, ClientTickPostEvent, ClientTooltipComponent, Component,
    DebugEntry, DebugGroup, DrawContext, EffectCategory, Flow, GameMode, GameRuleValue,
    GuiLayerPlacement, Hand, Hooks, Host, HostSide, HudMode, IPayloadContext, IPayloadHandler,
    ItemStack, JavaString, LivingRef, PayloadBytes, PayloadHandleError, PayloadId,
    PlayerLoggedInEvent, PlayerLoggedOutEvent, PlayerRef, PlayerTickPostEvent,
    RegisterClientTooltipComponentFactoriesEvent, RegisterDebugEntriesEvent,
    RegisterGuiLayersEvent, Registrar, RegistrationContext, RenderTooltipGatherComponentsEvent,
    ServerHost, ServerTickEvent, ServerTickPhase, SettingSpec, Tag, Text, TooltipComponent,
    TooltipComponentData, TooltipElement, TooltipFont,
};
use rules::{BarState, Body, Flash, Held, Offsets, Settings, Sprite};

const FOOD: &str = "minecraft:food";
const CONSUMABLE: &str = "minecraft:consumable";
const TOOLTIP_DISPLAY: &str = "minecraft:tooltip_display";
const NATURAL_REGENERATION_RULE: &str = "minecraft:natural_health_regeneration";
/// Where both bars start above the screen's bottom edge, before vanilla's layers move them
/// (`Gui.leftHeight` and `rightHeight`).
const BAR_HEIGHT: i32 = 39;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

static OFFSETS: Mutex<Option<Offsets>> = Mutex::new(None);
static FLASH: Mutex<Option<Flash>> = Mutex::new(None);
/// The held-food query of the last HUD tick it ran for.
static HELD: Mutex<Option<(i32, Option<Held>)>> = Mutex::new(None);
/// What the server last told this client.
static SYNCED: Mutex<Synced> = Mutex::new(Synced {
    saturation: None,
    exhaustion: None,
    natural_regeneration: true,
});

struct Synced {
    saturation: Option<f32>,
    exhaustion: Option<f32>,
    natural_regeneration: bool,
}

/// The client options (`ModConfig.CLIENT`), as the player set them in Options > Mods.
fn settings(host: &dyn ClientHost) -> Settings {
    Settings::read(|key| host.setting_bool(key), |key| host.setting_f64(key))
}

/// The options on the mod's page, labelled and explained as the original's config screen does.
fn register_settings(r: &mut dyn Registrar) {
    let label = |name: &str| format!("appleskin.configuration.{name}");
    let tooltip = |name: &str| format!("appleskin.configuration.{name}.tooltip");
    for (key, name, default) in rules::TOGGLES {
        let _ = r.register_setting(
            SettingSpec::toggle(key, label(name), default).tooltip(tooltip(name)),
        );
    }
    let (key, name, default) = rules::FLASH_ALPHA;
    let _ = r.register_setting(
        SettingSpec::range(key, label(name), 0.0, 1.0, 0.05, default).tooltip(tooltip(name)),
    );
}

// ---- reading the stack ----------------------------------------------------------------------

/// A component read as its raw tag.
struct Raw(&'static str);

impl Component for Raw {
    type Value = Tag;

    fn id(&self) -> &str {
        self.0
    }

    fn decode(&self, value: &Tag) -> Option<Tag> {
        Some(value.clone())
    }

    fn encode(&self, value: Tag) -> Tag {
        value
    }
}

/// `ItemStack.get`: the stack's own value, or the item's default unless the stack removes it.
fn component(host: &dyn Host, stack: &ItemStack, id: &'static str) -> Option<Tag> {
    if let Some(patch) = stack.components()
        && patch
            .as_nbt()
            .contains_key(&JavaString::from(format!("!{id}").as_str()))
    {
        return None;
    }
    stack
        .get(&Raw(id))
        .or_else(|| host.item_default_component(stack.item(), id))
}

/// `FoodHelper.query`: the food a stack is, when it has both `food` and `consumable`.
fn held_food(host: &dyn Host, stack: &ItemStack) -> Option<Held> {
    let food = component(host, stack, FOOD)?;
    let consumable = component(host, stack, CONSUMABLE)?;
    let effects = rules::consume_effects(&consumable);
    let rotten = effects.iter().flatten().any(|effect| {
        host.effect_key(&effect.id)
            .and_then(|key| key.category(host))
            == Some(EffectCategory::Harmful)
    });
    Some(Held {
        food: rules::Food::from_tag(&food),
        rotten,
        regeneration: rules::regeneration_of(&effects),
    })
}

// ---- the HUD -------------------------------------------------------------------------------

/// Everything a layer reads about the local player, once per frame.
struct Hud {
    player: PlayerRef,
    living: LivingRef,
    tick: i32,
    left: i32,
    right: i32,
    top: i32,
    food_level: i32,
    saturation: f32,
    exhaustion: f32,
}

/// `Overlay.render`'s opening: the local player, if the HUD applies to them.
fn hud(host: &dyn ClientHost, draw: &dyn DrawContext) -> Option<Hud> {
    let player = host.local_player()?;
    let can_hurt = matches!(
        player.game_mode(host),
        Some(GameMode::Survival | GameMode::Adventure)
    );
    if !rules::hud_applies(host.hud_mode() != HudMode::AllVisible, can_hurt) {
        return None;
    }
    let living = host.living(player.entity())?;
    let food = player.food(host)?;
    let [width, height] = draw.size();
    let synced = lock(&SYNCED);
    Some(Hud {
        player,
        living,
        tick: host.gui_ticks(),
        left: width as i32 / 2 - 91,
        right: width as i32 / 2 + 91,
        top: height as i32,
        food_level: food.level,
        saturation: synced.saturation.unwrap_or(food.saturation),
        exhaustion: synced.exhaustion.unwrap_or(food.exhaustion),
    })
}

impl Hud {
    /// `isMountHealthShown`: riding a living vehicle replaces the food bar with its health.
    fn on_living_mount(&self, host: &dyn ClientHost) -> bool {
        self.player
            .entity()
            .vehicle(host)
            .and_then(|vehicle| host.living(vehicle))
            .is_some()
    }

    fn bar_state(&self, host: &dyn ClientHost, animations: bool) -> BarState {
        BarState {
            max_health: self.living.max_health(host).unwrap_or(20.0),
            absorption: self.living.absorption(host).unwrap_or(0.0),
            health: self.living.health(host).unwrap_or(20.0),
            food_level: self.food_level,
            saturation: self.saturation,
            animations,
        }
    }

    fn offsets<R>(
        &self,
        host: &dyn ClientHost,
        settings: &Settings,
        read: impl FnOnce(&Offsets) -> R,
    ) -> R {
        let state = self.bar_state(host, settings.animations);
        let mut guard = lock(&OFFSETS);
        read(
            guard
                .get_or_insert_with(Offsets::default)
                .at(self.tick, &state),
        )
    }

    /// `HeldFoodCache.result`: the food in hand that the player could eat, asked once a HUD tick.
    fn held(&self, host: &dyn ClientHost, settings: &Settings) -> Option<Held> {
        let mut cache = lock(&HELD);
        if let Some((tick, held)) = cache.as_ref()
            && *tick == self.tick
        {
            return held.clone();
        }
        let query = |hand: Hand| {
            let held = self
                .living
                .with_hand_item(host, hand, |view| view.project())
                .flatten()
                .and_then(|stack| held_food(host, &stack))?;
            (self.player.can_eat(host, held.food.can_always_eat) == Some(true)).then_some(held)
        };
        let mut held = query(Hand::Main);
        if held.is_none() && settings.food_overlay_offhand {
            held = query(Hand::Off);
        }
        *cache = Some((self.tick, held.clone()));
        held
    }
}

fn draw_sprites(draw: &mut dyn DrawContext, sprites: &[Sprite]) {
    for sprite in sprites {
        draw.sprite(sprite.name, sprite.src, sprite.dst, Some(sprite.tint));
    }
}

fn flash() -> Flash {
    *lock(&FLASH).get_or_insert_with(Flash::default)
}

fn reset_flash() {
    lock(&FLASH).get_or_insert_with(Flash::default).reset();
}

/// The layers, each beside the vanilla layer whose bar it draws on.
mod layers {
    use super::*;

    const PLAYER_HEALTH: &str = "minecraft:player_health";
    const FOOD_LEVEL: &str = "minecraft:food_level";

    /// The anchor of both bars, in GUI pixels from the top of the screen.
    fn bar_y(hud: &Hud) -> i32 {
        hud.top - BAR_HEIGHT
    }

    /// `HealthOverlay`: the hearts the held food would heal.
    fn health(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
        let settings = settings(host);
        let Some(hud) = hud(host, draw) else { return };
        if hud.on_living_mount(host) {
            return;
        }
        // An offsets size of zero disables the overlay when health is infinite.
        if hud.offsets(host, &settings, |o| o.health.is_empty()) {
            return;
        }
        let Some(level) = hud.player.level(host) else {
            return;
        };
        let has = |id: &str| {
            host.effect_key(id)
                .is_some_and(|key| hud.living.effect(host, key).is_some())
        };
        if !rules::shows_estimated_health(
            settings.health_overlay,
            level.difficulty(host) == Some("peaceful"),
            hud.food_level,
            has("minecraft:poison") || has("minecraft:wither") || has("minecraft:regeneration"),
        ) {
            return;
        }
        let Some(held) = hud.held(host, &settings) else {
            reset_flash();
            return;
        };
        let health = hud.living.health(host).unwrap_or(0.0);
        let max_health = hud.living.max_health(host).unwrap_or(20.0);
        let body = Body {
            food_level: hud.food_level,
            saturation: hud.saturation,
            exhaustion: hud.exhaustion,
            hurt: health < max_health,
            natural_regeneration: lock(&SYNCED).natural_regeneration,
        };
        let modified = (health + rules::health_increment(&body, &held)).min(max_health);
        if health < modified {
            let sprites = hud.offsets(host, &settings, |o| {
                rules::health_sprites(
                    health,
                    modified,
                    hud.left,
                    bar_y(&hud),
                    flash().alpha,
                    false,
                    &o.health,
                )
            });
            draw_sprites(draw, &sprites);
        }
    }

    /// `HungerOverlay`: the hunger icons the held food would fill.
    fn hunger(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
        let settings = settings(host);
        let Some(hud) = hud(host, draw) else { return };
        if hud.on_living_mount(host) || !settings.food_overlay {
            return;
        }
        let Some(held) = hud.held(host, &settings) else {
            reset_flash();
            return;
        };
        let sprites = hud.offsets(host, &settings, |o| {
            rules::hunger_sprites(
                held.food.nutrition,
                hud.food_level,
                hud.right,
                bar_y(&hud),
                flash().alpha,
                held.rotten,
                &o.food,
            )
        });
        draw_sprites(draw, &sprites);
    }

    /// `SaturationOverlay`: the saturation, and what the held food adds to it.
    fn saturation(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
        let settings = settings(host);
        let Some(hud) = hud(host, draw) else { return };
        if hud.on_living_mount(host) || !settings.saturation_overlay {
            return;
        }
        let y = bar_y(&hud);
        let mut sprites = hud.offsets(host, &settings, |o| {
            rules::saturation_sprites(0.0, hud.saturation, hud.right, y, 1.0, &o.food)
        });
        if settings.food_overlay
            && let Some(held) = hud.held(host, &settings)
        {
            let new_food = hud.food_level + held.food.nutrition;
            let new_saturation = hud.saturation + held.food.saturation;
            let gained = if new_saturation > new_food as f32 {
                new_food as f32 - hud.saturation
            } else {
                held.food.saturation
            };
            sprites.extend(hud.offsets(host, &settings, |o| {
                rules::saturation_sprites(
                    gained,
                    hud.saturation,
                    hud.right,
                    y,
                    flash().alpha,
                    &o.food,
                )
            }));
        }
        draw_sprites(draw, &sprites);
    }

    /// `ExhaustionOverlay`: the bar behind the food icons.
    fn exhaustion(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
        let settings = settings(host);
        let Some(hud) = hud(host, draw) else { return };
        if hud.on_living_mount(host) || !settings.exhaustion_underlay {
            return;
        }
        let sprite = rules::exhaustion_sprite(hud.exhaustion, hud.right, bar_y(&hud));
        draw_sprites(draw, &[sprite]);
    }

    pub(super) fn register(event: &mut RegisterGuiLayersEvent<'_>) {
        let above = |id: &str| GuiLayerPlacement::Above(id.to_owned());
        event.layers.register(
            "appleskin:health_restored".into(),
            above(PLAYER_HEALTH),
            Box::new(health),
        );
        event.layers.register(
            "appleskin:hunger_restored".into(),
            above(FOOD_LEVEL),
            Box::new(hunger),
        );
        event.layers.register(
            "appleskin:saturation_level".into(),
            above(FOOD_LEVEL),
            Box::new(saturation),
        );
        event.layers.register(
            "appleskin:exhaustion_level".into(),
            GuiLayerPlacement::Below(FOOD_LEVEL.to_owned()),
            Box::new(exhaustion),
        );
    }
}

// ---- the tooltip ---------------------------------------------------------------------------

/// The hunger and saturation rows under a food's name (`FoodTooltip`, the tooltip component
/// data `TooltipOverlayHandler` adds).
struct FoodTooltip(rules::Tooltip);

impl TooltipComponentData for FoodTooltip {}

impl FoodTooltip {
    /// `shouldShowTooltip`, then `FoodTooltip` for the stack.
    fn of(host: &dyn ClientHost, stack: &ItemStack) -> Option<Self> {
        let hidden = component(host, stack, TOOLTIP_DISPLAY)
            .and_then(|display| match display.get("hide_tooltip") {
                Some(Tag::Byte(v)) => Some(*v != 0),
                _ => None,
            })
            .unwrap_or(false);
        if hidden || !settings(host).tooltip_wanted(host.modifiers().shift()) {
            return None;
        }
        let held = held_food(host, stack)?;
        let tooltip = rules::Tooltip::new(held.food, held.food, held.rotten);
        tooltip.shown().then_some(Self(tooltip))
    }
}

/// `ClientFoodTooltipComponent`: measures and draws a [`FoodTooltip`].
struct ClientFoodTooltip(rules::Tooltip);

impl ClientTooltipComponent for ClientFoodTooltip {
    fn height(&self, _: &dyn TooltipFont) -> i32 {
        rules::Tooltip::HEIGHT as i32
    }

    fn width(&self, font: &dyn TooltipFont) -> i32 {
        self.0
            .width(&|text| font.width(&Text::literal(text)).unwrap_or(0) as f32) as i32
    }

    fn draw_image(
        &self,
        _: &dyn TooltipFont,
        x: i32,
        y: i32,
        _: i32,
        _: i32,
        draw: &mut dyn DrawContext,
    ) {
        let (sprites, texts) = self.0.draw(x as f32, y as f32);
        draw_sprites(draw, &sprites);
        for (text, at, scale, argb) in texts {
            draw.text_scaled(&text, at, scale, argb, true);
        }
    }
}

// ---- the server's sync ------------------------------------------------------------------

/// What the server last sent each player.
#[derive(Default)]
struct Sent {
    saturation: Option<f32>,
    exhaustion: Option<f32>,
}

static SENT: Mutex<Option<HashMap<u128, Sent>>> = Mutex::new(None);
/// Whether natural regeneration was on when the server last looked.
static REGENERATION: Mutex<Option<bool>> = Mutex::new(None);

fn send(host: &mut dyn ServerHost, player: PlayerRef, channel: &str, bytes: Vec<u8>) {
    // A player that has left, or a client without the mod, simply gets nothing.
    let _ = player.send_payload(host, &PayloadId(channel.into()), PayloadBytes(bytes));
}

fn natural_regeneration(host: &dyn ServerHost) -> bool {
    !matches!(
        host.game_rule(NATURAL_REGENERATION_RULE),
        Ok(GameRuleValue::Bool(false))
    )
}

/// The channels' handlers: each stores what the server says in [`SYNCED`].
struct FromServer(&'static str);

impl IPayloadHandler for FromServer {
    fn handle(
        &self,
        payload: PayloadBytes,
        _: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let unreadable = || PayloadHandleError::Failed(format!("an unreadable {}", self.0));
        let mut synced = lock(&SYNCED);
        match self.0 {
            rules::CHANNEL_SATURATION => {
                synced.saturation = Some(rules::decode_float(&payload.0).ok_or_else(unreadable)?);
            }
            rules::CHANNEL_EXHAUSTION => {
                synced.exhaustion = Some(rules::decode_float(&payload.0).ok_or_else(unreadable)?);
            }
            _ => {
                synced.natural_regeneration =
                    rules::decode_flag(&payload.0).ok_or_else(unreadable)?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::sunrise_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        register_settings(r);
        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");
        for channel in [
            rules::CHANNEL_SATURATION,
            rules::CHANNEL_EXHAUSTION,
            rules::CHANNEL_NATURAL_REGENERATION,
        ] {
            view.play_to_client(
                PayloadId(channel.into()),
                Some(Arc::new(FromServer(channel))),
            );
        }
    }

    fn register_client_tooltip_component_factories(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterClientTooltipComponentFactoriesEvent<'_>,
    ) {
        event
            .factories
            .register(|data: &FoodTooltip| ClientFoodTooltip(data.0.clone()));
    }

    /// `TooltipOverlayHandler.gatherTooltips`: the food rows go after the tooltip's lines.
    fn render_tooltip_gather_components(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipGatherComponentsEvent<'_>,
    ) -> Flow {
        if let Some(tooltip) = FoodTooltip::of(host, event.stack) {
            event
                .elements
                .push(TooltipElement::Component(TooltipComponent::custom(tooltip)));
        }
        Flow::Continue
    }

    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        layers::register(event);
    }

    fn register_debug_entries(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterDebugEntriesEvent<'_>,
    ) {
        event.entries.push(
            DebugEntry::text(
                "appleskin:food_stats".into(),
                DebugGroup::Position,
                |host, lines| {
                    let Some(player) = host.local_player() else {
                        return;
                    };
                    let Some(food) = player.food(host) else {
                        return;
                    };
                    lines.push(rules::debug_line(
                        food.level,
                        food.saturation,
                        food.exhaustion,
                    ));
                },
            )
            .with_reduced_debug_info(false),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let max = settings(&*host).max_flash_alpha;
        lock(&FLASH).get_or_insert_with(Flash::default).step(max);
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let player = event.player;
        let Some(uuid) = player.entity().uuid(host) else {
            return;
        };
        let Some(food) = player.food(&*host) else {
            return;
        };
        let (saturation, exhaustion) = {
            let mut map = lock(&SENT);
            let sent = map
                .get_or_insert_with(HashMap::new)
                .entry(uuid)
                .or_default();
            let saturation = (sent.saturation != Some(food.saturation)).then_some(food.saturation);
            let exhaustion = rules::exhaustion_changed(sent.exhaustion, food.exhaustion)
                .then_some(food.exhaustion);
            if saturation.is_some() {
                sent.saturation = saturation;
            }
            if exhaustion.is_some() {
                sent.exhaustion = Some(food.exhaustion);
            }
            (saturation, exhaustion)
        };
        if let Some(value) = saturation {
            send(
                host,
                player,
                rules::CHANNEL_SATURATION,
                rules::encode_float(value),
            );
        }
        if let Some(value) = exhaustion {
            send(
                host,
                player,
                rules::CHANNEL_EXHAUSTION,
                rules::encode_float(value),
            );
        }
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        lock(&SENT)
            .get_or_insert_with(HashMap::new)
            .remove(&event.player);
        let Some(player) = host.player(event.player) else {
            return;
        };
        let flag = rules::encode_flag(natural_regeneration(&*host));
        send(host, player, rules::CHANNEL_NATURAL_REGENERATION, flag);
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        lock(&SENT)
            .get_or_insert_with(HashMap::new)
            .remove(&event.player);
    }

    fn client_player_logging_in(&self, _: &mut dyn ClientHost, _: &mut ClientPlayerLoggingInEvent) {
        *lock(&SYNCED) = Synced {
            saturation: None,
            exhaustion: None,
            natural_regeneration: true,
        };
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            return;
        }
        let now = natural_regeneration(&*host);
        let changed = {
            let mut last = lock(&REGENERATION);
            let changed = last.is_some_and(|last| last != now);
            *last = Some(now);
            changed
        };
        if !changed {
            return;
        }
        let players = host.players().server_players();
        for player in players {
            send(
                host,
                player,
                rules::CHANNEL_NATURAL_REGENERATION,
                rules::encode_flag(now),
            );
        }
    }
}
