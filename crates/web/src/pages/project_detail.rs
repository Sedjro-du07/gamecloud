//! Project detail page.
//!
//! The interesting half of this page is the verdict table. A project is
//! judged once per concerned track, and a rejection is required to
//! carry written feedback — so this is where a member actually finds
//! out what to fix, from the specialist who would know.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::{
    api::{ProjectRights, VerdictItem},
    components::review_form::ReviewForm,
    server_fns::{get_project, submit_project},
};

/// French label and CSS modifier for a verdict.
fn verdict_style(status: &str) -> (&'static str, &'static str) {
    match status {
        "Approved" => ("Approuvé", "gc-verdict--ok"),
        "Rejected" => ("Refusé", "gc-verdict--no"),
        "NotApplicable" => ("Non applicable", "gc-verdict--na"),
        _ => ("En attente", "gc-verdict--pending"),
    }
}

/// Render the per-track verdict table.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn VerdictTable(validations: Vec<VerdictItem>) -> impl IntoView {
    if validations.is_empty() {
        return view! {
            <p class="gc-empty">"Ce projet n'a pas encore été soumis à la revue."</p>
        }
        .into_any();
    }

    view! {
        <div class="gc-table-wrap">
            <table class="gc-table">
                <thead>
                    <tr>
                        <th scope="col">"Track"</th>
                        <th scope="col">"Verdict"</th>
                        <th scope="col">"Relecteur"</th>
                        <th scope="col">"Retour"</th>
                    </tr>
                </thead>
                <tbody>
                    {validations
                        .into_iter()
                        .map(|v| {
                            let (label, modifier) = verdict_style(&v.status);
                            view! {
                                <tr>
                                    <td>{v.track}</td>
                                    <td>
                                        <span class=format!("gc-verdict {modifier}")>{label}</span>
                                    </td>
                                    <td>{v.reviewer_name.unwrap_or_else(|| "—".into())}</td>
                                    <td class="gc-table__feedback">
                                        {v.feedback.unwrap_or_else(|| "—".into())}
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
    .into_any()
}

/// Everything the viewer is entitled to do here.
///
/// The flags come from the server; this only decides what to draw. Each
/// button calls an endpoint that re-checks the same right, so a crafted
/// request gains nothing.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ActionPanel(
    project_id: String,
    rights: ProjectRights,
    on_changed: Callback<String>,
) -> impl IntoView {
    let submit = Action::new(move |id: &String| {
        let id = id.clone();
        async move { submit_project(id).await }
    });
    let (notice, set_notice) = signal(Option::<String>::None);

    Effect::new(move |_| {
        if let Some(result) = submit.value().get() {
            match result {
                Ok(tracks) => {
                    set_notice
                        .set(Some(format!("Revue ouverte auprès de : {}", tracks.join(", "))));
                    on_changed.run("InReview".to_string());
                }
                Err(e) => set_notice.set(Some(e.to_string())),
            }
        }
    });

    let nothing_to_do = rights.reviewable_tracks.is_empty()
        && !rights.can_submit
        && !rights.can_release;
    if nothing_to_do {
        return ().into_any();
    }

    let id_for_submit = project_id.clone();
    view! {
        <section class="gc-actions">
            <h2>"Vos actions"</h2>
            {move || {
                notice.get().map(|n| view! { <div class="gc-banner">{n}</div> })
            }}

            <Show when=move || rights.can_submit>
                <button
                    class="gc-btn gc-btn--primary"
                    disabled=move || submit.pending().get()
                    on:click={
                        let id = id_for_submit.clone();
                        move |_| { submit.dispatch(id.clone()); }
                    }
                >
                    {move || {
                        if submit.pending().get() { "Soumission…" } else { "Soumettre en revue" }
                    }}
                </button>
                <p class="gc-actions__hint">
                    "Chaque track concernée sera sollicitée dans son propre salon Discord."
                </p>
            </Show>

            <Show when=move || rights.can_release>
                <p class="gc-actions__hint">
                    "Ce projet est approuvé. En tant que responsable de la track
                     principale, vous pouvez le publier — l'équipe sera récompensée
                     et le dépôt GitHub passera public."
                </p>
            </Show>

            {rights
                .reviewable_tracks
                .clone()
                .into_iter()
                .map(|track| {
                    view! {
                        <ReviewForm
                            project_id=project_id.clone()
                            track=track
                            on_done=on_changed
                        />
                    }
                })
                .collect_view()}
        </section>
    }
        .into_any()
}

/// The project, once loaded.
///
/// Split out of [`ProjectDetailPage`] so that component stays a thin
/// load-state switch and this one owns the layout.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ProjectBody(
    /// The loaded project.
    detail: crate::api::ProjectDetailView,
    /// Refetch hook, called after an action changes the project.
    on_changed: Callback<String>,
) -> impl IntoView {
    let card = detail.card.clone();
    view! {
        <>
            <header
                class="gc-project-detail__head"
                style=format!("--gc-rarity: {};", card.rarity_color)
            >
                <p class="gc-project-detail__track">
                    {card.track_emoji} " " {card.track}
                </p>
                <h1>{card.name}</h1>
                {card.short_description.map(|d| view! { <p class="gc-lead">{d}</p> })}
                <p class="gc-project-detail__meta">
                    {card.rarity} " · " {card.status} " · "
                    {format!("{} contributeur(s)", card.contributor_count)}
                </p>
            </header>

            {detail.long_description.map(|d| view! { <p class="gc-project-detail__desc">{d}</p> })}

            <div class="gc-project-detail__links">
                {detail
                    .github_repo_url
                    .map(|url| {
                        view! {
                            <a class="gc-btn" href=url rel="external noopener">"Dépôt GitHub"</a>
                        }
                    })}
                {detail
                    .itch_url
                    .map(|url| {
                        view! {
                            <a class="gc-btn" href=url rel="external noopener">"Page itch.io"</a>
                        }
                    })}
            </div>

            <h2>"Équipe"</h2>
            <ul class="gc-contributors">
                {detail
                    .contributors
                    .into_iter()
                    .map(|c| {
                        view! {
                            <li class="gc-contributor">
                                <span class="gc-contributor__name">{c.display_name}</span>
                                <span class="gc-contributor__role">
                                    {c.track}
                                    {c.role.map(|r| format!(" · {r}"))}
                                </span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>

            <h2>"Revue par track"</h2>
            <VerdictTable validations=detail.validations />

            <ActionPanel project_id=card.id rights=detail.rights on_changed />
        </>
    }
}

/// Project detail page.
#[component]
pub fn ProjectDetailPage() -> impl IntoView {
    let params = use_params_map();
    let project = Resource::new(
        move || params.read().get("id").unwrap_or_default(),
        |id| async move { get_project(id).await },
    );
    let on_changed = Callback::new(move |_status: String| project.refetch());

    view! {
        <section class="gc-project-detail">
            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match project.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="gc-empty">"Impossible de charger ce projet."</p> }
                            .into_any()
                    }
                    Some(Ok(None)) => {
                        view! {
                            <p class="gc-empty">
                                "Projet introuvable. "
                                <a href="/projects">"Retour au Hall of Fame"</a>
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(Some(detail))) => {
                        view! { <ProjectBody detail on_changed /> }.into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::verdict_style;

    #[test]
    fn every_schema_verdict_has_a_label() {
        // Exactly the values allowed by track_validations_status_valid.
        for status in ["Pending", "Approved", "Rejected", "NotApplicable"] {
            let (label, modifier) = verdict_style(status);
            assert!(!label.is_empty());
            assert!(modifier.starts_with("gc-verdict--"));
        }
    }

    #[test]
    fn an_unknown_verdict_reads_as_pending() {
        // Failing closed: never show an unrecognised verdict as approval.
        assert_eq!(verdict_style("Maybe").0, "En attente");
    }
}
