//! Inspiring quotes — a primitive and its catalogue.
//!
//! A quote of the day, chosen from the date so the server and the browser
//! agree on it, and offset per placement so two places on one screen never
//! show the same one. Attributions are kept to what the author is known to
//! have said or written, with the work when there is one; the association's
//! own mottos are signed by the platform.

use leptos::{either::Either, prelude::*};

use super::{
    button::{Button, ButtonKind},
    icon::IconName,
};

/// One quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Citation {
    /// The words, in French.
    pub text: &'static str,
    /// Who said or wrote them.
    pub author: &'static str,
    /// Where, when known.
    pub source: Option<&'static str>,
}

const fn q(text: &'static str, author: &'static str, source: Option<&'static str>) -> Citation {
    Citation { text, author, source }
}

/// The catalogue.
pub const QUOTES: &[Citation] = &[
    // Play and game design.
    q("Un jeu est une suite de choix intéressants.", "Sid Meier", Some("GDC, 1989")),
    q("S'amuser n'est qu'une autre façon de dire apprendre.", "Raph Koster", Some("A Theory of Fun for Game Design")),
    q("Jouer à un jeu, c'est tenter volontairement de surmonter des obstacles inutiles.", "Bernard Suits", Some("The Grasshopper")),
    q("Le jeu est plus ancien que la culture.", "Johan Huizinga", Some("Homo Ludens")),
    q("Le jeu n'est pas l'expérience : il rend l'expérience possible.", "Jesse Schell", Some("L'Art du game design")),
    q(
        "Sur ma carte de visite, je suis président d'entreprise. Dans ma tête, je suis développeur de jeux. Mais dans mon cœur, je suis un joueur.",
        "Satoru Iwata",
        Some("GDC, 2005"),
    ),
    // Code and craft.
    q("Fais que ça marche, puis fais-le bien, puis fais-le vite.", "Kent Beck", None),
    q("L'optimisation prématurée est la source de tous les maux.", "Donald Knuth", None),
    q("Ajouter des gens à un projet logiciel en retard le retarde encore.", "Fred Brooks", Some("Le Mythe du mois-homme")),
    q("Parler ne coûte rien. Montre-moi le code.", "Linus Torvalds", None),
    q("La simplicité est la condition de la fiabilité.", "Edsger W. Dijkstra", None),
    q("La meilleure façon de prédire l'avenir, c'est de l'inventer.", "Alan Kay", None),
    q(
        "N'importe quel idiot peut écrire du code qu'un ordinateur comprend. Les bons programmeurs écrivent du code que les humains comprennent.",
        "Martin Fowler",
        Some("Refactoring"),
    ),
    q(
        "Les programmes doivent être écrits pour être lus par des humains, et accessoirement pour être exécutés par des machines.",
        "Harold Abelson et Gerald Sussman",
        Some("Structure and Interpretation of Computer Programs"),
    ),
    q("Les vrais artistes livrent.", "Steve Jobs", None),
    q("Restez affamés, restez fous.", "Steve Jobs", Some("discours de Stanford, 2005")),
    // Making things.
    q(
        "La perfection est atteinte non quand il n'y a plus rien à ajouter, mais quand il n'y a plus rien à retrancher.",
        "Antoine de Saint-Exupéry",
        Some("Terre des hommes"),
    ),
    q(
        "Pour ce qui est de l'avenir, il ne s'agit pas de le prévoir, mais de le rendre possible.",
        "Antoine de Saint-Exupéry",
        Some("Citadelle"),
    ),
    q("Vingt fois sur le métier remettez votre ouvrage.", "Nicolas Boileau", Some("L'Art poétique")),
    q(
        "Ce que l'on conçoit bien s'énonce clairement, et les mots pour le dire arrivent aisément.",
        "Nicolas Boileau",
        Some("L'Art poétique"),
    ),
    q(
        "Je n'ai fait celle-ci plus longue que parce que je n'ai pas eu le loisir de la faire plus courte.",
        "Blaise Pascal",
        Some("Les Provinciales"),
    ),
    q("L'inspiration existe, mais elle doit te trouver au travail.", "Pablo Picasso", None),
    q("L'imagination est plus importante que le savoir.", "Albert Einstein", None),
    q("Le hasard ne favorise que les esprits préparés.", "Louis Pasteur", None),
    q(
        "Le génie, c'est un pour cent d'inspiration et quatre-vingt-dix-neuf pour cent de transpiration.",
        "Thomas Edison",
        None,
    ),
    q(
        "L'échec n'est que l'occasion de recommencer, plus intelligemment.",
        "Henry Ford",
        Some("Ma vie et mon œuvre"),
    ),
    // Daring and the long road.
    q(
        "Ce n'est pas parce que les choses sont difficiles que nous n'osons pas, c'est parce que nous n'osons pas qu'elles sont difficiles.",
        "Sénèque",
        Some("Lettres à Lucilius"),
    ),
    q("Un voyage de mille lieues commence par un premier pas.", "Lao-tseu", Some("Tao-tö-king")),
    // Together.
    q("Seul, on va plus vite ; ensemble, on va plus loin.", "Proverbe africain", None),
    q("Quand les toiles d'araignée s'unissent, elles peuvent attacher un lion.", "Proverbe éthiopien", None),
    q("Un seul bracelet ne tinte pas.", "Proverbe africain", None),
    // The association's own.
    q("Chaque commit est une ligne de plus dans ta légende.", "GameCloud OS", None),
    q("Un projet rendu vaut mieux que dix projets rêvés.", "GameCloud OS", None),
    q("Relire le travail d'un autre, c'est progresser deux fois.", "GameCloud OS", None),
    q("Une séance de plus, un titre plus près.", "GameCloud OS", None),
    q("Le meilleur jeu est celui que tu finis.", "GameCloud OS", None),
];

/// Today's day number, the same on the server and in the browser.
fn day_number() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp().div_euclid(86_400)).unwrap_or(0)
}

/// The quote of the day for one placement. Different `salt`s give different
/// quotes on the same day.
///
/// Fixed-width arithmetic on purpose: the server is 64-bit and the browser's
/// WebAssembly 32-bit, and a `usize` that wraps differently on each would
/// pick two different quotes and break hydration.
#[must_use]
pub fn quote_of_day(salt: u32) -> Citation {
    let len = QUOTES.len() as u64;
    let i = (day_number() * 7 + u64::from(salt) * 13) % len;
    QUOTES[usize::try_from(i).unwrap_or(0)]
}

/// A placement number from a page address, identical on the server and in
/// the browser.
#[must_use]
pub fn salt_for(path: &str) -> u32 {
    path.bytes().fold(5_u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)))
}

/// A quote: the words, then who said them.
#[component]
pub fn Quote(
    /// Which placement this is: different numbers, different quotes.
    #[prop(optional)]
    salt: u32,
    /// One discreet line, for a footer.
    #[prop(optional)]
    compact: bool,
    /// Offer another quote on demand.
    #[prop(optional)]
    shuffle: bool,
) -> impl IntoView {
    let (offset, set_offset) = signal(0_usize);
    let current = move || {
        let base = quote_of_day(salt);
        let start = QUOTES.iter().position(|c| *c == base).unwrap_or(0);
        QUOTES[(start + offset.get()) % QUOTES.len()]
    };
    let class = if compact { "ui-quote ui-quote--compact" } else { "ui-quote" };
    view! {
        <figure class=class>
            {move || {
                let c = current();
                // The same shape with or without a source — an author, then a
                // `cite` that is hidden when empty — so the page the server
                // sends and the one the browser hydrates always match.
                let source = c.source.map_or_else(
                    || Either::Right(view! { <cite class="ui-quote__source" hidden=true></cite> }),
                    |source| Either::Left(view! { <cite class="ui-quote__source">{source}</cite> }),
                );
                view! {
                    <blockquote class="ui-quote__text"><p>{c.text}</p></blockquote>
                    <figcaption class="ui-quote__by"><span class="ui-quote__author">{c.author}</span>{source}</figcaption>
                }
            }}
            {shuffle.then(|| view! {
                <div class="ui-quote__more">
                    <Button kind=ButtonKind::Ghost icon=IconName::Lightbulb on:click=move |_| set_offset.update(|o| *o += 1)>
                        "Autre citation"
                    </Button>
                </div>
            })}
        </figure>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_quote_is_signed_and_written() {
        for c in QUOTES {
            assert!(!c.text.trim().is_empty());
            assert!(!c.author.trim().is_empty());
            assert!(c.text.ends_with(['.', '!', '?']), "{}", c.text);
        }
    }

    #[test]
    fn a_page_address_picks_the_same_quote_everywhere() {
        // The same number on a 64-bit server and in 32-bit WebAssembly.
        assert_eq!(salt_for("/admin"), salt_for("/admin"));
        assert_eq!(salt_for("/"), 5 * 31 + u32::from(b'/'));
    }

    #[test]
    fn two_placements_differ_on_the_same_day() {
        assert_ne!(quote_of_day(0), quote_of_day(1));
        assert_ne!(quote_of_day(1), quote_of_day(2));
    }
}
