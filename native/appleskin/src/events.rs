//! The events other Sunrise mods can hear from AppleSkin: the typed counterparts of the original's
//! `FoodValuesEvent` and `HUDOverlayEvent` (NeoForge's bus is Sunrise's `host.post` / `#[subscribe]`).
//!
//! A mod listens by declaring `mod-appleskin` under `[dependencies]` and subscribing:
//!
//! ```ignore
//! #[subscribe]
//! fn double_food(&self, _: &mut dyn mod_api::ClientHost, ev: &mut appleskin::events::FoodValuesEvent) {
//!     ev.modified_saturation *= 2.0;
//! }
//! ```
//!
//! Both are client events, posted once per client tick for the food the local player holds.

/// One of the overlays AppleSkin draws over the vanilla bars (`HUDOverlayEvent`'s subclasses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    /// The hunger icons the held food would fill (`HUDOverlayEvent.HungerRestored`).
    HungerRestored,
    /// The saturation and what the held food adds to it (`HUDOverlayEvent.Saturation`).
    Saturation,
    /// The exhaustion bar behind the food icons (`HUDOverlayEvent.Exhaustion`).
    Exhaustion,
    /// The hearts the held food would heal (`HUDOverlayEvent.HealthRestored`).
    HealthRestored,
}

impl Overlay {
    /// Every overlay, in the order the layers draw them.
    pub const ALL: [Overlay; 4] = [
        Overlay::HungerRestored,
        Overlay::Saturation,
        Overlay::Exhaustion,
        Overlay::HealthRestored,
    ];

    /// The overlay's place in [`Overlay::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Overlay::HungerRestored => 0,
            Overlay::Saturation => 1,
            Overlay::Exhaustion => 2,
            Overlay::HealthRestored => 3,
        }
    }
}

/// What the food in the local player's hand gives, which a listener may change
/// (`FoodValuesEvent`). The HUD overlays show `modified_*`; the defaults are the food's own.
#[derive(mod_api::Event)]
#[event(id = "appleskin:food_values", side = client)]
#[non_exhaustive]
pub struct FoodValuesEvent {
    /// Whether the food is in the off hand.
    pub offhand: bool,
    /// The hunger points the food restores.
    pub default_nutrition: i32,
    /// The saturation the food adds.
    pub default_saturation: f32,
    /// The hunger points the overlays show; starts as the default.
    pub modified_nutrition: i32,
    /// The saturation the overlays show; starts as the default.
    pub modified_saturation: f32,
}

impl FoodValuesEvent {
    /// An event whose modified values start as the defaults.
    pub fn new(offhand: bool, nutrition: i32, saturation: f32) -> Self {
        Self {
            offhand,
            default_nutrition: nutrition,
            default_saturation: saturation,
            modified_nutrition: nutrition,
            modified_saturation: saturation,
        }
    }
}

/// An overlay about to be drawn this tick; a listener that cancels it hides that overlay
/// (`HUDOverlayEvent`, which is cancelled the same way).
#[derive(mod_api::Event)]
#[event(id = "appleskin:hud_overlay", side = client, cancellable)]
#[non_exhaustive]
pub struct HudOverlayEvent {
    /// Which overlay this is.
    pub overlay: Overlay,
    /// The player's saturation.
    pub saturation: f32,
    /// The player's exhaustion.
    pub exhaustion: f32,
    /// Whether a listener hid the overlay.
    pub cancelled: bool,
}

impl HudOverlayEvent {
    /// An event for `overlay`, not cancelled.
    pub fn new(overlay: Overlay, saturation: f32, exhaustion: f32) -> Self {
        Self {
            overlay,
            saturation,
            exhaustion,
            cancelled: false,
        }
    }
}

/// Which overlays a listener hid on the last tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hidden([bool; 4]);

impl Hidden {
    /// Records the outcome of `overlay`'s event.
    pub fn set(&mut self, overlay: Overlay, hidden: bool) {
        self.0[overlay.index()] = hidden;
    }

    /// Whether `overlay` is hidden.
    pub fn is(&self, overlay: Overlay) -> bool {
        self.0[overlay.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_food_values_start_as_the_defaults() {
        let ev = FoodValuesEvent::new(true, 6, 7.2);
        assert!(ev.offhand);
        assert_eq!(ev.modified_nutrition, ev.default_nutrition);
        assert_eq!(ev.modified_saturation, 7.2);
    }

    #[test]
    fn a_new_overlay_event_is_not_cancelled() {
        let ev = HudOverlayEvent::new(Overlay::Saturation, 5.0, 1.0);
        assert!(!ev.cancelled);
        assert_eq!(ev.overlay, Overlay::Saturation);
    }

    #[test]
    fn overlay_indices_follow_all() {
        for (i, overlay) in Overlay::ALL.into_iter().enumerate() {
            assert_eq!(overlay.index(), i);
        }
    }

    #[test]
    fn hiding_one_overlay_leaves_the_others() {
        let mut hidden = Hidden::default();
        hidden.set(Overlay::Exhaustion, true);
        assert!(hidden.is(Overlay::Exhaustion));
        assert!(!hidden.is(Overlay::HungerRestored));
        hidden.set(Overlay::Exhaustion, false);
        assert_eq!(hidden, Hidden::default());
    }
}
