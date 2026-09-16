//! Primitives of the interface.
//!
//! Screens are composed from these and nothing else: the three page
//! patterns (dense list, card grid, detail page) and their header, filter
//! bar and sections; button, card, field, tag, icon; notice, avatar,
//! conversation, form, figure, sound; the XP bar, streak marks and title ladder;
//! and the loading, empty, error and signed-out states. Their look lives in
//! `style/_primitives.scss`, built only from the tokens in
//! `style/_tokens.scss`. Every one is shown on `/design`.

pub mod avatar;
pub mod button;
pub mod card;
pub mod chat;
pub mod field;
pub mod figure;
pub mod form;
pub mod icon;
pub mod ladder;
pub mod layout;
pub mod list;
pub mod notice;
pub mod segmented;
pub mod sound;
pub mod states;
pub mod tag;
pub mod vocab;
pub mod xp;

pub use avatar::{Avatar, AvatarSize};
pub use button::{Button, ButtonKind, ButtonLink};
pub use card::{Card, CardMedia, CardSkeleton};
pub use chat::{ChatLog, Message, MessageSide};
pub use field::Field;
pub use figure::{Figure, Visual};
pub use form::{Form, FormActions, FormRow};
pub use icon::{Icon, IconName, IconSize};
pub use ladder::TitleLadder;
pub use layout::{
    CardGrid, Cluster, Disclosure, Fact, Facts, FilterBar, Gap, Page, PageHeader, Panel, Pattern,
    Section, Split, Stack, Step, Stepper, Steps,
};
pub use list::{DenseList, ListGroup, ListRow, RowText, RowValue};
pub use notice::{Notice, NoticeKind};
pub use segmented::{segment, segment_with_icon, Segment, SegmentedControl};
pub use sound::{play, Sound, SoundToggle};
pub use states::{
    EmptyState, ErrorState, ErrorText, GridSkeleton, MembersOnlyState, PageSkeleton, RowsSkeleton,
    SignInState,
};
pub use tag::{Tag, TagKind, TrackTag};
pub use xp::{StreakMarks, XpProgress};

/// A visual state forced on a primitive, so the `/design` preview can show
/// hover, pressed and focus without a pointer. Screens never set it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DemoState {
    /// The real state, driven by the pointer and keyboard.
    #[default]
    Live,
    /// Shown as hovered.
    Hover,
    /// Shown as pressed.
    Pressed,
    /// Shown with keyboard focus.
    Focus,
}

impl DemoState {
    /// The class suffix that forces the state, with its leading space.
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::Live => "",
            Self::Hover => " is-hover",
            Self::Pressed => " is-pressed",
            Self::Focus => " is-focus",
        }
    }
}
