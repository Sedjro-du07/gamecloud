//! Preview of the redesign's tokens and primitives.
//!
//! Not in the menu: it exists so the direction can be validated before any
//! screen is rebuilt. Every primitive is shown in each of its states; the
//! `is-*` states are forced, since a static page has no pointer. Example
//! content is labelled as such and uses the platform's own wording.

use leptos::prelude::*;

use crate::components::ui::{
    segment, Avatar, Button, ButtonKind, ButtonLink, Card, CardGrid, CardMedia, CardSkeleton,
    ChatLog, Cluster, DemoState, DenseList, EmptyState, ErrorState, ErrorText, Fact, Facts, Field,
    FilterBar, Form, FormActions, GridSkeleton, Icon, IconName, IconSize, ListGroup, ListRow,
    Message, MessageSide, Notice, NoticeKind, PageHeader, Panel, RowText, RowValue, RowsSkeleton,
    SegmentedControl, SignInState, StreakMarks, Tag, TagKind, TitleLadder, TrackTag, Visual,
    XpProgress,
};
use gamecloud_shared::roles::GlobalRank;
use crate::components::ui::{play, Sound};

/// Token name, value as declared, role.
type Swatch = (&'static str, &'static str, &'static str);

const GROUNDS: [Swatch; 6] = [
    ("--bg", "#070812", "fond, sous l'image"),
    ("--glass", "rgba(12,15,34,.66)", "panneaux en verre"),
    ("--text", "#e8ecff", "texte principal"),
    ("--text-muted", "#aeb6dc", "texte secondaire"),
    ("--text-faint", "#8a93c0", "méta"),
    ("--border", "rgba(0,242,255,.2)", "bordures néon"),
];

const ACCENT: [Swatch; 5] = [
    ("--accent", "#00f2ff", "néon cyan : traits, icônes, lueurs"),
    ("--accent-2", "#bf00ff", "néon magenta : dégradés, obtenu"),
    ("--accent-300", "#8ff9ff", "texte accentué"),
    ("--accent-800", "rgba(0,242,255,.22)", "survol, sélection"),
    ("--accent-900", "rgba(0,242,255,.09)", "teinte discrète"),
];

const NEUTRALS: [Swatch; 9] = [
    ("--n-50", "#f3f5fe", ""),
    ("--n-100", "#e4e7f5", ""),
    ("--n-200", "#cfd3e5", ""),
    ("--n-300", "#b2b6ca", ""),
    ("--n-400", "#9397ab", ""),
    ("--n-500", "#75798c", ""),
    ("--n-600", "#595d6c", ""),
    ("--n-700", "#3f424d", ""),
    ("--n-800", "#292b31", ""),
];

const SPACING: [(&str, &str); 8] = [
    ("--space-1", "2.8"),
    ("--space-2", "5.6"),
    ("--space-3", "8.4"),
    ("--space-4", "11.2"),
    ("--space-5", "16.8"),
    ("--space-6", "22.4"),
    ("--space-7", "44.8"),
    ("--space-8", "67.2"),
];

/// A row of colour swatches.
#[component]
fn Swatches(
    /// The colours.
    items: &'static [Swatch],
) -> impl IntoView {
    view! {
        <div class="ui-swatches">
            {items
                .iter()
                .map(|(token, value, role)| {
                    view! {
                        <div class="ui-swatch">
                            <span class="ui-swatch__chip" style=format!("background: var({token})")></span>
                            <span class="ui-swatch__name">{*token}</span>
                            <span class="ui-swatch__value">
                                {*value}
                                {(!role.is_empty()).then(|| format!(" · {role}"))}
                            </span>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// A section heading.
#[component]
fn SectionHead(
    /// Small capitals above the title.
    label: &'static str,
    /// Title.
    title: &'static str,
    /// What to look at.
    #[prop(optional)]
    note: Option<&'static str>,
) -> impl IntoView {
    view! {
        <header class="ui-preview__head">
            <span class="ui-label">{label}</span>
            <h2 class="ui-h2">{title}</h2>
            {note.map(|n| view! { <p class="ui-body ui-muted">{n}</p> })}
        </header>
    }
}

/// One primitive shown in one state, with the state named under it.
#[component]
fn State(
    /// The state's name.
    name: &'static str,
    /// The primitive.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="ui-preview__state">
            {children()}
            <span class="ui-meta">{name}</span>
        </div>
    }
}

/// Buttons in every kind and state.
#[component]
fn ButtonsSection() -> impl IntoView {
    let kinds = [
        (ButtonKind::Primary, "Primaire", "Rendre mon travail"),
        (ButtonKind::Secondary, "Secondaire", "Voir les rendus"),
        (ButtonKind::Ghost, "Ghost", "Annuler"),
    ];
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Primitive" title="Bouton"
                note="Une bordure, jamais un aplat. Le primaire porte l'accent en trait et en texte." />
            {kinds
                .into_iter()
                .map(|(kind, name, text)| {
                    view! {
                        <div class="ui-preview__group">
                            <h3 class="ui-h3">{name}</h3>
                            <div class="ui-preview__row">
                                <State name="repos"><Button kind>{text}</Button></State>
                                <State name="survol"><Button kind state=DemoState::Hover>{text}</Button></State>
                                <State name="pressé"><Button kind state=DemoState::Pressed>{text}</Button></State>
                                <State name="focus clavier"><Button kind state=DemoState::Focus>{text}</Button></State>
                                <State name="désactivé"><Button kind disabled=true>{text}</Button></State>
                            </div>
                        </div>
                    }
                })
                .collect_view()}
            <div class="ui-preview__group">
                <h3 class="ui-h3">"Avec icône"</h3>
                <div class="ui-preview__row">
                    <Button kind=ButtonKind::Primary icon=IconName::Plus>"Ouvrir un test"</Button>
                    <Button icon=IconName::DownloadSimple>"Télécharger"</Button>
                    <Button kind=ButtonKind::Ghost trailing_icon=IconName::ArrowRight>"Tout le calendrier"</Button>
                </div>
            </div>
        </section>
    }
}

/// Tags, including the three quest states.
#[component]
fn TagsSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Primitive" title="Tag"
                note="12 px, majuscules, contour. L'état se dit par l'icône, pas par la couleur." />
            <div class="ui-preview__row">
                <State name="titre du membre"><Tag kind=TagKind::Accent>"Ancien de la Forge"</Tag></State>
                <State name="catégorie"><Tag>"Réunion"</Tag></State>
                <State name="quête à faire"><Tag icon=IconName::Circle>"À faire"</Tag></State>
                <State name="quête en cours"><Tag icon=IconName::CircleHalf>"En cours"</Tag></State>
                <State name="quête validée"><Tag kind=TagKind::Accent icon=IconName::CheckCircle>"Validée"</Tag></State>
                <State name="réservé"><Tag icon=IconName::LockSimple>"Membres"</Tag></State>
            </div>
        </section>
    }
}

/// Fields in every state.
#[component]
fn FieldsSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Primitive" title="Champ"
                note="Élément natif, label au-dessus, un peu plus sombre que la surface. Pas de rouge : une erreur se dit par une icône et des mots." />
            <div class="ui-preview__fields">
                <Field id="demo-title" label="Titre" hint="120 caractères maximum.">
                    <input id="demo-title" class="ui-control" type="text"
                        placeholder="Test d'entrée — Engineering" aria-describedby="demo-title-hint" />
                </Field>
                <Field id="demo-focus" label="Durée, en heures">
                    <input id="demo-focus" class="ui-control is-focus" type="number" value="72" />
                </Field>
                <Field id="demo-error" label="Lien" error="Le lien doit commencer par https://">
                    <input id="demo-error" class="ui-control" type="url" value="itch.io/jam"
                        aria-invalid="true" aria-describedby="demo-error-error" />
                </Field>
                <Field id="demo-disabled" label="Pseudo Discord" disabled=true>
                    <input id="demo-disabled" class="ui-control" type="text" disabled=true
                        placeholder="franckalain07" />
                </Field>
                <Field id="demo-select" label="Track">
                    <select id="demo-select" class="ui-control">
                        <option>"Toutes"</option>
                        <option>"Engineering"</option>
                        <option>"Game Design"</option>
                    </select>
                </Field>
                <Field id="demo-file" label="Sujet (PDF)">
                    <input id="demo-file" class="ui-control" type="file" accept=".pdf" />
                </Field>
                <Field id="demo-notes" label="Consignes (optionnel)">
                    <textarea id="demo-notes" class="ui-control" rows="3"
                        placeholder="Ce qu'il faut rendre, sous quelle forme…"></textarea>
                </Field>
            </div>
        </section>
    }
}

/// Cards in every state.
#[component]
fn CardsSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Primitive" title="Carte"
                note="Kicker, titre, corps, méta. Validée : bordure accent et coche, jamais un fond coloré. Exemples tirés des libellés de la plateforme." />
            <div class="ui-preview__grid">
                <State name="repos">
                    <Card kicker="Exemple · quête" title="Relire un projet" meta="+50 XP · jusqu'au 21/09">
                        "Donner l'avis de ta track sur un projet en attente."
                    </Card>
                </State>
                <State name="lien, survol">
                    <Card kicker="Exemple · track" title="Engineering" href="/design" state=DemoState::Hover
                        meta="100 XP avant Contributeur">
                        "Gameplay, outils, moteur."
                    </Card>
                </State>
                <State name="validée">
                    <Card kicker="Exemple · palier" title="Apprenti Forgeron" achieved=true meta="150 XP">
                        "Palier franchi."
                    </Card>
                </State>
                <State name="avec visuel (à fournir)">
                    <Card kicker="Exemple · projet" title="Visuel du projet" media=CardMedia::Pending
                        meta="Le visuel réel remplace ce cadre.">
                        "Photo sur fond sombre, fondue dans la page."
                    </Card>
                </State>
                <State name="chargement">
                    <CardSkeleton />
                </State>
            </div>
        </section>
    }
}

/// The header and filter bar every screen opens with.
#[component]
fn HeaderSection() -> impl IntoView {
    let (filter, set_filter) = signal("open".to_string());
    let filters = view! {
        <FilterBar label="Exemple de filtres">
            <SegmentedControl
                label="État"
                options=vec![segment("open", "En cours"), segment("done", "Validées"), segment("all", "Toutes")]
                value=filter
                on_change=Callback::new(move |v| set_filter.set(v))
            />
        </FilterBar>
    }
    .into_any();
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Primitive" title="En-tête d'écran et barre de filtres"
                note="Le même sur tous les écrans : h1, une phrase de contexte, les actions à droite du titre, puis les filtres en contrôle segmenté." />
            <Panel>
                <PageHeader title="Quêtes" lead="Exemple : le Bureau les propose, elles se valident toutes seules." filters>
                    <Button kind=ButtonKind::Primary icon=IconName::Plus>"Ouvrir une quête"</Button>
                </PageHeader>
            </Panel>
        </section>
    }
}

/// The dense list, one row per item.
#[component]
fn ListSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Patron" title="Liste dense"
                note="Quêtes, Ressources, Classement, Calendrier. Une ligne par élément : tête (état, rang, heure), titre et méta, valeur ou actions au bout. Pas de carte par élément." />
            <DenseList label="Exemple de liste">
                <ListGroup label="Exemple · mardi 16" />
                <ListRow lead=view! { <Icon name=IconName::CircleHalf /> }.into_any() title="Relire trois projets"
                    meta="Rendre des revues · 1 / 3 · encore 4 jours" end=view! { <RowValue text="+40 XP" /> }.into_any()>
                    <Cluster><TrackTag track="GameDesign" /></Cluster>
                </ListRow>
                <ListRow lead=view! { <Icon name=IconName::Circle /> }.into_any() title="Venir à deux séances"
                    meta="Être présent · 0 / 2" end=view! { <RowValue text="+30 XP" /> }.into_any() />
                <ListRow lead=view! { <span>"3"</span><Avatar name="Ada" src=None /> }.into_any() title="Ada"
                    meta="Compagnon de Guilde · 4 jours de série" end=view! { <RowValue text="1 240 XP" /> }.into_any() />
                <ListRow lead=view! { <Icon name=IconName::LockSimple /> }.into_any() title="Oiseau de nuit"
                    meta="Ligne atténuée : passée, verrouillée, annulée." dimmed=true>
                    <RowText text="Un texte sous le titre : description, retour de relecture." />
                </ListRow>
            </DenseList>
        </section>
    }
}

/// The card grid.
#[component]
fn GridSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Patron" title="Grille de cartes"
                note="Projets, Partages, Tracks. Sans capture, la surface et l'icône de la track : jamais un cadre vide." />
            <CardGrid>
                <Card kicker="Exemple · Engineering" title="Projet sans capture" media=CardMedia::Missing(IconName::Code)
                    meta="Publié · Rare · 3 contributeurs" href="/design">
                    "La carte entière est un lien."
                </Card>
                <Card kicker="Exemple · Script" icon=IconName::FileCode title="Partage" meta="par Ada · hier · 12 téléchargements"
                    actions=view! { <Button kind=ButtonKind::Primary icon=IconName::DownloadSimple>"Télécharger"</Button> }.into_any()>
                    "Une carte avec des actions n'est pas un lien."
                </Card>
                <Card title="Audio" icon=IconName::MusicNotes kicker="Rejointe" achieved=true>"Composition · Sound design"</Card>
            </CardGrid>
            <Visual src=None alt="Exemple" icon=IconName::Cube />
        </section>
    }
}

/// Notices, forms, avatar, conversation, facts, ladder.
#[component]
fn DetailSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Patron" title="Page de détail au fer à gauche"
                note="Profil, Tests d'entrée, Contact, Responsabilités. Une colonne, des sections, des formulaires en colonnes." />
            <div class="ui-preview__group">
                <Notice>"Information : un trait d'accent, jamais un aplat."</Notice>
                <Notice kind=NoticeKind::Success>"Rendu reçu. Tu peux le remplacer jusqu'à la fin du test."</Notice>
                <Notice kind=NoticeKind::Error>"Le fichier dépasse 500 Mo."</Notice>
            </div>
            <Panel>
                <Form>
                    <Field id="demo-form-titre" label="Titre"><input id="demo-form-titre" class="ui-control" type="text" /></Field>
                    <Field id="demo-form-type" label="Type"><select id="demo-form-type" class="ui-control"><option>"Script"</option></select></Field>
                    <FormActions>
                        <Button kind=ButtonKind::Primary icon=IconName::UploadSimple>"Partager"</Button>
                        <Button kind=ButtonKind::Ghost>"Annuler"</Button>
                    </FormActions>
                </Form>
                <ErrorText message="Texte d'erreur : icône et mots, 13 px." />
            </Panel>
            <Cluster>
                <Avatar name="Ada" src=None size=crate::components::ui::AvatarSize::Large />
                <Tag>"Grand Archonte"</Tag>
                <Tag kind=TagKind::Accent>"Maître Artisan"</Tag>
                <Tag>"Relecteur · Audio"</Tag>
            </Cluster>
            <XpProgress percent=62.0 caption="Exemple · 1 930 XP · encore 570 XP avant Maître Artisan" />
            <Cluster><StreakMarks days=4 /><span class="ui-meta">"4 jours de série · XP ×1,1"</span></Cluster>
            <Facts>
                <Fact label="XP total" value="1 930" />
                <Fact label="Titre suivant" value="Maître Artisan" />
                <Fact label="Classement" value="3ᵉ" />
            </Facts>
            <ChatLog label="Exemple de conversation">
                <Message side=MessageSide::Typing meta="Kumo" body="Kumo va répondre…" />
                <Message side=MessageSide::Own meta="Toi · 10:02 · transmis à Kumo" body="Quand ouvre le prochain test ?" />
                <Message meta="Kumo · 09:58" body="Bonjour ! Pose ta question ici." />
            </ChatLog>
            <TitleLadder current=GlobalRank::JuniorDev />
        </section>
    }
}

/// Loading, empty, error, signed out.
#[component]
fn StatesSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="États" title="Chargement, vide, erreur, déconnecté"
                note="Chaque écran en montre un au lieu d'une zone blanche ou d'un spinner." />
            <div class="ui-preview__grid">
                <div class="ui-preview__state"><RowsSkeleton rows=3 /><span class="ui-meta">"chargement · liste"</span></div>
                <div class="ui-preview__state">
                    <EmptyState icon=IconName::Target title="Aucune quête en cours" body="Le Bureau en publie chaque semaine." />
                    <span class="ui-meta">"vide"</span>
                </div>
                <div class="ui-preview__state">
                    <ErrorState message="Impossible de charger les quêtes." on_retry=Callback::new(|()| ()) />
                    <span class="ui-meta">"erreur"</span>
                </div>
                <div class="ui-preview__state"><SignInState what="voir le calendrier" /><span class="ui-meta">"déconnecté"</span></div>
            </div>
            <GridSkeleton cards=3 />
            <Cluster>
                <ButtonLink href="/quests" icon=IconName::Target>"Voir un écran réel"</ButtonLink>
            </Cluster>
        </section>
    }
}

/// Every sound cue, to hear them.
#[component]
fn SoundsSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Primitive" title="Sons"
                note="Synthétisés dans le navigateur, aucun fichier. Joués par les primitives, donc les mêmes sur tous les écrans ; muets avant la première interaction, et coupables depuis la barre du haut." />
            <div class="ui-preview__row">
                {Sound::ALL
                    .into_iter()
                    .map(|sound| view! {
                        <button type="button" class="ui-btn" on:click=move |_| play(sound)>
                            <Icon name=IconName::SpeakerHigh />
                            {sound.label()}
                        </button>
                    })
                    .collect_view()}
            </div>
        </section>
    }
}

/// Tokens: colours, type, spacing, radii, elevation.
#[component]
fn TokensSection() -> impl IntoView {
    view! {
        <section class="ui-preview__section">
            <SectionHead label="Tokens" title="Couleurs"
                note="Un seul accent. Tout le reste en neutres désaturés." />
            <div class="ui-preview__group"><h3 class="ui-h3">"Fonds et texte"</h3><Swatches items=&GROUNDS /></div>
            <div class="ui-preview__group"><h3 class="ui-h3">"Accent"</h3><Swatches items=&ACCENT /></div>
            <div class="ui-preview__group"><h3 class="ui-h3">"Neutres"</h3><Swatches items=&NEUTRALS /></div>
        </section>

        <section class="ui-preview__section">
            <SectionHead label="Tokens" title="Typographie"
                note="Inter partout, titres en 500 au plus. La police du wordmark ne sert qu'au wordmark." />
            <div>
                <div class="ui-type-row"><span class="ui-meta">"wordmark"</span><span class="ui-wordmark ui-h2">"GameCloud OS"</span></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"display · 40 / 1.1"</span><span class="ui-display">"Ancien de la Forge"</span></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"h1 · 32"</span><span class="ui-h1">"Quêtes"</span></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"h2 · 24"</span><span class="ui-h2">"Prochaine séance"</span></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"h3 · 18"</span><span class="ui-h3">"Tests d'entrée"</span></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"corps · 15 / 1.6"</span>
                    <p class="ui-body">"Code, crée, viens aux séances : chaque action te rapporte de l'XP, et l'XP te fait monter en titre."</p></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"méta · 13"</span><span class="ui-meta">"17/09/2026 · 10:00"</span></div>
                <hr class="ui-divider" />
                <div class="ui-type-row"><span class="ui-meta">"label · 12"</span><span class="ui-label">"Association"</span></div>
            </div>
        </section>

        <section class="ui-preview__section">
            <SectionHead label="Tokens" title="Espacement, rayons, élévation"
                note="Échelle dense 0,7×. Élévation : 1 px de bord et obscurité ambiante, jamais de lueurs empilées." />
            <div class="ui-scale">
                {SPACING
                    .iter()
                    .map(|(token, px)| view! {
                        <div class="ui-scale__row">
                            <span class="ui-meta">{format!("{token} · {px} px")}</span>
                            <span class="ui-scale__bar" style=format!("width: var({token})")></span>
                        </div>
                    })
                    .collect_view()}
            </div>
            <div class="ui-preview__grid">
                <div class="ui-preview__state"><div class="ui-radius" style="border-radius: var(--radius-sm)"></div><span class="ui-meta">"--radius-sm · 4 px · champs, tags"</span></div>
                <div class="ui-preview__state"><div class="ui-radius" style="border-radius: var(--radius-md)"></div><span class="ui-meta">"--radius-md · 8 px · cartes, boutons"</span></div>
                <div class="ui-preview__state"><div class="ui-radius" style="border-radius: var(--radius-lg)"></div><span class="ui-meta">"--radius-lg · 14 px · modales"</span></div>
            </div>
            <div class="ui-preview__grid">
                <div class="ui-elev" style="box-shadow: var(--elev-sm)"><span class="ui-h3">"sm"</span><span class="ui-meta">"bord 1 px"</span></div>
                <div class="ui-elev" style="box-shadow: var(--elev-md)"><span class="ui-h3">"md"</span><span class="ui-meta">"bord + ombre 18 px"</span></div>
                <div class="ui-elev" style="box-shadow: var(--elev-lg)"><span class="ui-h3">"lg"</span><span class="ui-meta">"bord + ombre 40 px, modales"</span></div>
            </div>
        </section>
    }
}

/// The preview page.
#[component]
pub fn DesignPage() -> impl IntoView {
    // The shell provides the page ground.
    view! {
        <div>
            <div class="ui-preview">
                <header class="ui-preview__intro">
                    <span class="ui-label">"Système de design · HUD néon"</span>
                    <h1 class="ui-display">"Tokens et primitives"</h1>
                    <p class="ui-body ui-muted">
                        "Un HUD de science-fiction posé sur une image de fond : néons cyan et magenta,
                         panneaux en verre, titres en Orbitron. Chaque primitive est montrée dans
                         chacun de ses états."
                    </p>
                    <div class="ui-preview__row">
                        <Icon name=IconName::MagnifyingGlass size=IconSize::Medium />
                        <span class="ui-meta">"Icônes Phosphor, trait régulier, 16 ou 20 px, couleur héritée."</span>
                    </div>
                </header>
                <TokensSection />
                <ButtonsSection />
                <TagsSection />
                <FieldsSection />
                <CardsSection />
                <HeaderSection />
                <ListSection />
                <GridSection />
                <DetailSection />
                <StatesSection />
                <SoundsSection />
            </div>
        </div>
    }
}
