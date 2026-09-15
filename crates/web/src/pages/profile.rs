//! Profile — the member's record, as a detail page.
//!
//! Every title held (office, member title, track titles, in that order), the
//! XP bar to the next title, the streak, the tracks, the badges and the
//! ledger of what they did. It prints to a clean PDF: a record worth
//! something outside the club.

use gamecloud_shared::roles::{GlobalRank, TrackRole};
use leptos::prelude::*;

use crate::{
    api::{format_xp, level_percent, BadgeItem, SheetView, TrackView, XpEntry},
    components::ui::{
        vocab::{badge_icon, plain_title, role_title, track_icon_by_id, track_name},
        xp::{rank_percent, streak_caption, xp_caption},
        Avatar, AvatarSize, Button, ButtonLink, Cluster, DenseList, EmptyState, ErrorState, Fact,
        Facts, Icon, IconName, ListRow, Notice, Page, PageHeader, PageSkeleton, Pattern, RowValue,
        Section, SignInState, Stack, StreakMarks, Tag, TagKind, TitleLadder, XpProgress,
    },
    server_fns::get_sheet,
};

/// The member's tracks, each with the bar to its next title.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Tracks(
    /// Tracks joined.
    tracks: Vec<TrackView>,
) -> impl IntoView {
    if tracks.is_empty() {
        return view! {
            <EmptyState icon=IconName::Compass title="Aucune track rejointe" body="Une track, et l'XP de track commence à compter.">
                <ButtonLink href="/tracks" icon=IconName::Plus>"Choisir mes tracks"</ButtonLink>
            </EmptyState>
        }
        .into_any();
    }
    view! {
        <DenseList label="Tracks">
            {tracks
                .into_iter()
                .map(|t| {
                    let lead = view! { <Icon name=track_icon_by_id(&t.id).unwrap_or(IconName::Compass) /> }.into_any();
                    let title = role_title(&t.role);
                    let meta = t.specialization.map_or_else(|| title.to_string(), |s| format!("{title} · {s}"));
                    let end = view! { <RowValue text=format!("{} XP", format_xp(t.xp)) /> }.into_any();
                    // From Mentor up, titles are appointed: no bar.
                    let progress = TrackRole::next_milestone(&t.role).map(|(next, floor, target)| {
                        let pct = level_percent((t.xp - floor).max(0), target - floor);
                        let caption = format!("{} XP avant {}", format_xp((target - t.xp).max(0)), plain_title(next));
                        view! { <XpProgress percent=pct caption /> }
                    });
                    view! { <ListRow lead title=track_name(&t.id) meta end>{progress}</ListRow> }
                })
                .collect_view()}
        </DenseList>
    }
    .into_any()
}

/// Every badge, earned ones first; the others show what is worth doing.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Badges(
    /// The catalogue, flagged.
    badges: Vec<BadgeItem>,
) -> impl IntoView {
    let mut badges = badges;
    badges.sort_by_key(|b| !b.held);
    view! {
        <DenseList label="Badges">
            {badges
                .into_iter()
                .map(|b| {
                    let lead = view! { <Icon name=badge_icon(&b.id) /> }.into_any();
                    let end = if b.held {
                        view! { <Tag icon=IconName::CheckCircle>"Obtenu"</Tag> }.into_any()
                    } else {
                        view! { <Tag icon=IconName::LockSimple>"À débloquer"</Tag> }.into_any()
                    };
                    view! { <ListRow lead title=plain_title(&b.title).to_string() meta=b.description end dimmed=!b.held /> }
                })
                .collect_view()}
        </DenseList>
    }
}

/// Recent ledger lines.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn History(
    /// Lines, newest first.
    entries: Vec<XpEntry>,
) -> impl IntoView {
    if entries.is_empty() {
        return view! {
            <EmptyState icon=IconName::Lightning title="Rien pour l'instant"
                body="Pousse du code, viens à une séance ou relis le projet de quelqu'un." />
        }
        .into_any();
    }
    view! {
        <DenseList label="Activité récente">
            {entries
                .into_iter()
                .map(|e| {
                    let mut meta = format!("{} · {}", e.when, e.source);
                    if let Some(track) = &e.track {
                        meta.push_str(" · ");
                        meta.push_str(&track_name(track));
                    }
                    let title = e.description.unwrap_or_else(|| e.source.clone());
                    let end = view! { <RowValue text=format!("{:+} XP", e.amount) /> }.into_any();
                    view! { <ListRow title meta end /> }
                })
                .collect_view()}
        </DenseList>
    }
    .into_any()
}

/// The profile, once loaded.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn SheetBody(
    /// The sheet.
    sheet: SheetView,
) -> impl IntoView {
    let me = sheet.me;
    // Every title held, most important first: office, member title, tracks.
    let office = me.bureau_title.as_deref().map(|o| plain_title(o).to_string());
    let title = plain_title(&me.rank_title).to_string();
    let track_titles: Vec<String> = sheet
        .tracks
        .iter()
        .map(|t| format!("{} · {}", role_title(&t.role), track_name(&t.id)))
        .collect();
    let next_title = me.next_rank_title.as_deref().map_or_else(|| "—".to_string(), |t| plain_title(t).to_string());
    let place = me.email_verified.then_some(me.leaderboard_position).flatten().map(|p| format!("{p}ᵉ"));
    let held = sheet.badges.iter().filter(|b| b.held).count();
    let total = sheet.badges.len();
    let rank = GlobalRank::parse(&me.global_rank);
    let (percent, caption, streak) = (rank_percent(&me), xp_caption(&me), streak_caption(&me));
    let (name, avatar, total_xp, streak_days) = (me.display_name.clone(), me.avatar_url.clone(), format_xp(me.xp_total), me.streak_days);
    let verified = me.email_verified;

    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader title=name.clone() kicker="Profil" lead="Ton parcours dans l'association, prêt à joindre à une candidature.">
                <Button icon=IconName::Printer attr:onclick="window.print()">"Imprimer ou enregistrer en PDF"</Button>
            </PageHeader>
            {(!verified).then(|| view! {
                <Notice>
                    <span>"Ton adresse Epitech n'est pas encore vérifiée."</span>
                    <ButtonLink href="/onboarding/email" external=true trailing_icon=IconName::ArrowRight>"Vérifier"</ButtonLink>
                </Notice>
            })}
            <Stack>
                <Cluster>
                    <Avatar name src=avatar size=AvatarSize::Large />
                    {office.map(|o| view! { <Tag>{o}</Tag> })}
                    <Tag kind=TagKind::Accent>{title}</Tag>
                    {track_titles.into_iter().map(|t| view! { <Tag>{t}</Tag> }).collect_view()}
                </Cluster>
                <XpProgress percent caption />
                <Cluster>
                    <StreakMarks days=streak_days />
                    <span class="ui-meta">{streak}</span>
                </Cluster>
                <Facts>
                    <Fact label="XP total" value=total_xp />
                    <Fact label="Titre suivant" value=next_title />
                    {place.map(|p| view! { <Fact label="Classement" value=p /> })}
                </Facts>
            </Stack>
            <Section title="Tracks"><Tracks tracks=sheet.tracks /></Section>
            <Section title="Badges" meta=format!("{held} / {total}")><Badges badges=sheet.badges /></Section>
            <Section title="Activité récente"><History entries=sheet.recent_xp /></Section>
            <Section title="Échelle des titres"><TitleLadder current=rank /></Section>
        </Page>
    }
}

/// Profile page.
#[component]
pub fn ProfilePage() -> impl IntoView {
    let sheet = Resource::new(|| (), |()| async { get_sheet().await });
    view! {
        <Suspense fallback=|| view! { <Page pattern=Pattern::Detail><PageSkeleton /></Page> }>
            {move || sheet.get().map(|result| match result {
                Err(_) => view! {
                    <Page pattern=Pattern::Detail>
                        <PageHeader title="Profil" />
                        <ErrorState message="Impossible de charger ton profil." on_retry=Callback::new(move |()| sheet.refetch()) />
                    </Page>
                }
                .into_any(),
                Ok(None) => view! {
                    <Page pattern=Pattern::Detail>
                        <PageHeader title="Profil" />
                        <SignInState what="voir ton profil" />
                    </Page>
                }
                .into_any(),
                Ok(Some(sheet)) => view! { <SheetBody sheet /> }.into_any(),
            })}
        </Suspense>
    }
}
