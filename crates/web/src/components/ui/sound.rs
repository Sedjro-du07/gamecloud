//! Sound — the game heard, not only seen.
//!
//! Short cues synthesised in the browser (`sound.js`, Web Audio API): a
//! click, a screen change, success, XP earned, a new title, an error, an
//! answer from Kumo. Primitives play them, so every screen sounds the same.
//! Nothing plays before the member's first interaction, and the top bar
//! switches them off for good.

use leptos::prelude::*;

use super::icon::{Icon, IconName};

/// A sound cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// A control clicked.
    Tick,
    /// Moving to another screen.
    Nav,
    /// The main action of a screen.
    Confirm,
    /// A menu or a panel opens.
    Open,
    /// It worked.
    Success,
    /// XP earned.
    Xp,
    /// A new title, a track joined.
    LevelUp,
    /// It did not work.
    Error,
    /// Kumo answered.
    Message,
}

impl Sound {
    /// Every cue, for the design preview.
    pub const ALL: [Self; 9] = [
        Self::Tick,
        Self::Nav,
        Self::Confirm,
        Self::Open,
        Self::Success,
        Self::Xp,
        Self::LevelUp,
        Self::Error,
        Self::Message,
    ];

    /// The cue's name in `sound.js`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::Nav => "nav",
            Self::Confirm => "confirm",
            Self::Open => "open",
            Self::Success => "success",
            Self::Xp => "xp",
            Self::LevelUp => "levelup",
            Self::Error => "error",
            Self::Message => "message",
        }
    }

    /// What the cue means, in words.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tick => "Clic",
            Self::Nav => "Changer d'écran",
            Self::Confirm => "Action principale",
            Self::Open => "Ouverture",
            Self::Success => "Réussite",
            Self::Xp => "XP gagnée",
            Self::LevelUp => "Nouveau titre",
            Self::Error => "Erreur",
            Self::Message => "Réponse de Kumo",
        }
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/components/ui/sound.js")]
extern "C" {
    #[wasm_bindgen(js_name = playSound)]
    fn play_sound(name: &str);
    #[wasm_bindgen(js_name = soundEnabled)]
    fn sound_enabled() -> bool;
    #[wasm_bindgen(js_name = setSoundEnabled)]
    fn set_sound_enabled(on: bool);
}

/// Play a cue. Silent on the server, when switched off, and before the
/// member's first interaction.
pub fn play(sound: Sound) {
    #[cfg(feature = "hydrate")]
    play_sound(sound.name());
    #[cfg(not(feature = "hydrate"))]
    let _ = sound;
}

/// Whether sounds are on in this browser.
fn enabled() -> bool {
    #[cfg(feature = "hydrate")]
    return sound_enabled();
    #[cfg(not(feature = "hydrate"))]
    true
}

/// Remember the choice in this browser.
fn set_enabled(on: bool) {
    #[cfg(feature = "hydrate")]
    set_sound_enabled(on);
    #[cfg(not(feature = "hydrate"))]
    let _ = on;
}

/// The switch in the top bar.
#[component]
pub fn SoundToggle() -> impl IntoView {
    // Rendered "on" everywhere, then set from the browser's memory.
    let (on, set_on) = signal(true);
    Effect::new(move |_| set_on.set(enabled()));
    let label = move || if on.get() { "Couper le son" } else { "Activer le son" };
    view! {
        <button
            type="button"
            class="ui-btn ui-btn--ghost"
            aria-pressed=move || on.get().to_string()
            title=label
            on:click=move |_| {
                let next = !on.get_untracked();
                set_enabled(next);
                set_on.set(next);
                if next {
                    play(Sound::Confirm);
                }
            }
        >
            {move || {
                if on.get() {
                    view! { <Icon name=IconName::SpeakerHigh /> }.into_any()
                } else {
                    view! { <Icon name=IconName::SpeakerSlash /> }.into_any()
                }
            }}
            <span class="ui-sr-only">{label}</span>
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::Sound;

    #[test]
    fn every_cue_has_its_own_name_and_a_label() {
        for (i, sound) in Sound::ALL.iter().enumerate() {
            assert!(!sound.label().is_empty());
            assert!(Sound::ALL[i + 1..].iter().all(|other| other.name() != sound.name()));
        }
    }
}
