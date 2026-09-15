//! Project creation — a detail page.
//!
//! Creating a project also creates its GitHub repository — private, with
//! the author as collaborator and the push webhook installed. That is why
//! the page says so: a member who has not linked their GitHub login gets a
//! repository they cannot push to, and this is the one moment they read it.

use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};

use crate::{
    components::ui::{
        vocab::track_choices, Button, ButtonKind, ErrorText, Field, Form, FormActions, FormRow,
        IconName, Notice, Page, PageHeader, Pattern,
    },
    server_fns::create_project,
};

/// Project creation page.
#[component]
pub fn NewProjectPage() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (summary, set_summary) = signal(String::new());
    let (track, set_track) = signal("Engineering".to_string());
    let (level, set_level) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let navigate = use_navigate();

    let create = Action::new(move |(n, s, t, l): &(String, String, String, String)| {
        let (n, s, t, l) = (n.clone(), s.clone(), t.clone(), l.clone());
        async move { create_project(n, s, t, l).await }
    });
    Effect::new(move |_| {
        if let Some(result) = create.value().get() {
            match result {
                Ok(id) => navigate(&format!("/projects/{id}"), NavigateOptions::default()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Nouveau projet"
                lead="Le projet démarre en brouillon. Un dépôt GitHub privé est créé dans l'organisation de l'association ; il devient public le jour où le projet est publié."
            />
            <Notice>
                "Lie ton compte GitHub à ton profil avant de commencer : sans ça, tu ne seras pas ajouté au dépôt et tes commits ne rapporteront pas d'XP."
            </Notice>
            <Form on:submit=move |ev| {
                ev.prevent_default();
                if name.get().trim().is_empty() {
                    set_error.set(Some("Le nom est obligatoire.".into()));
                    return;
                }
                create.dispatch((name.get(), summary.get(), track.get(), level.get()));
            }>
                <Field id="projet-nom" label="Nom du projet">
                    <input id="projet-nom" class="ui-control" type="text" required=true placeholder="Aevaryn"
                        prop:value=move || name.get() on:input=move |ev| set_name.set(event_target_value(&ev)) />
                </Field>
                <Field id="projet-resume" label="En une phrase">
                    <input id="projet-resume" class="ui-control" type="text" placeholder="Un RPG narratif en pixel art"
                        prop:value=move || summary.get() on:input=move |ev| set_summary.set(event_target_value(&ev)) />
                </Field>
                <Field id="projet-track" label="Track principale">
                    <select id="projet-track" class="ui-control" prop:value=move || track.get()
                        on:change=move |ev| set_track.set(event_target_value(&ev))>
                        {track_choices()
                            .into_iter()
                            .map(|(id, label)| view! { <option value=id>{label}</option> })
                            .collect_view()}
                    </select>
                </Field>
                <Field id="projet-niveau" label="Niveau Epitech (optionnel)">
                    <select id="projet-niveau" class="ui-control" on:change=move |ev| set_level.set(event_target_value(&ev))>
                        <option value="">"Aucun"</option>
                        <option value="Tek1">"Tek1"</option>
                        <option value="Tek2">"Tek2"</option>
                        <option value="Tek3">"Tek3"</option>
                        <option value="Master">"Master"</option>
                    </select>
                </Field>
                {move || error.get().map(|message| view! { <FormRow><ErrorText message /></FormRow> })}
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Plus
                        disabled=Signal::derive(move || create.pending().get())>
                        {move || if create.pending().get() { "Création…" } else { "Créer le projet" }}
                    </Button>
                </FormActions>
            </Form>
        </Page>
    }
}
