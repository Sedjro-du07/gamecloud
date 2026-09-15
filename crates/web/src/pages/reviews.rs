//! Review queue — the work waiting for you.
//!
//! A reviewer only sees projects in tracks where they hold `Reviewer` or
//! above, and never their own work. That filtering happens in SQL, not
//! here: a queue showing things you have no standing to judge is noise,
//! and hiding them in the browser would not stop anyone acting on them.

use leptos::prelude::*;

use crate::{
    components::ui::{
        ButtonKind, ButtonLink, Cluster, DenseList, EmptyState, ErrorState, IconName, ListRow,
        Page, PageHeader, Pattern, RowText, RowsSkeleton, TrackTag,
    },
    server_fns::get_review_queue,
};

/// Review queue page.
#[component]
pub fn ReviewsPage() -> impl IntoView {
    let queue = Resource::new(|| (), |()| async { get_review_queue().await });

    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="À relire"
                lead="Les projets qui attendent l'avis d'une track où tu es relecteur ou au-dessus. Un refus explique ce qui doit changer : c'est ce retour qui fait progresser l'équipe."
            />
            <Suspense fallback=|| view! { <RowsSkeleton rows=4 /> }>
                {move || {
                    queue
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! {
                                    <ErrorState
                                        message="Impossible de charger la file de relecture."
                                        on_retry=Callback::new(move |()| queue.refetch())
                                    />
                                }
                                    .into_any()
                            }
                            Ok(items) if items.is_empty() => {
                                view! {
                                    <EmptyState
                                        icon=IconName::Eye
                                        title="Rien à relire pour l'instant"
                                        body="Soit tout est à jour, soit tu n'es relecteur sur aucune track : 300 XP dans une track te le font devenir."
                                    />
                                }
                                    .into_any()
                            }
                            Ok(items) => {
                                view! {
                                    <DenseList label="Projets à relire">
                                        {items
                                            .into_iter()
                                            .map(|item| {
                                                let href = format!("/projects/{}", item.project_id);
                                                let end = view! {
                                                    <ButtonLink kind=ButtonKind::Ghost href=href.clone() trailing_icon=IconName::ArrowRight>
                                                        "Relire"
                                                    </ButtonLink>
                                                }
                                                    .into_any();
                                                view! {
                                                    <ListRow
                                                        title=item.name
                                                        href
                                                        meta=format!("Soumis par {}", item.author_name)
                                                        end
                                                    >
                                                        <Cluster><TrackTag track=item.track /></Cluster>
                                                        {item.short_description.map(|text| view! { <RowText text /> })}
                                                    </ListRow>
                                                }
                                            })
                                            .collect_view()}
                                    </DenseList>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </Page>
    }
}
