# Changelog

## Septembre 2026 (suite) — tests d'entrée, admission, contacter Kumo

### Tests d'entrée

Une page **🎓 Tests d'entrée**, visible seulement par le Bureau et par les
comptes qui ne sont pas encore membres vérifiés. Un membre ordinaire ne la
voit pas, ni dans le menu ni par l'adresse.

- **Le Bureau** ouvre un test : titre, consignes, durée en heures et sujet
  en PDF (un faux PDF est refusé). Il télécharge les rendus, **admet** ou
  **refuse** chaque candidat, peut clore un test plus tôt ou le supprimer
  avec son sujet et ses rendus.
- **Les candidats** téléchargent le sujet et rendent un fichier, avec un
  mot pour le Bureau, remplaçable jusqu'à la fin du test. Ils voient
  ensuite le résultat.

Aucun GitHub : les fichiers restent sur le serveur, dans `TESTS_DIR`
(par défaut `data/tests`).

Une personne extérieure les trouve sans compte : lien « 🎓 Tests
d'entrée » dans le menu, bouton sur l'accueil, et une page qui explique
comment rejoindre l'association et liste les tests ouverts (titre,
consignes, date de fin). Le sujet et le rendu demandent de se connecter.

### Admission

S'inscrire demande désormais d'être admis, sauf pour qui est déjà sur le
serveur Discord :

- à la connexion, un compte non vérifié qui est déjà sur le serveur
  s'inscrit comme avant ;
- sinon, c'est un **candidat** : il arrive sur les tests d'entrée, et la
  vérification de l'adresse Epitech lui est refusée ;
- quand le Bureau l'admet, une **invitation au serveur** à usage unique,
  valable 7 jours, est créée automatiquement et s'affiche sur sa page
  Tests. L'inscription s'ouvre ensuite.

Si Discord ne répond pas à la connexion, le statut déjà connu est gardé.
La plateforme lit `DISCORD_TOKEN`, `DISCORD_GUILD_ID` et
`DISCORD_INVITE_CHANNEL_ID`.

### Contacter Kumo

Un onglet **💬 Contacter Kumo**, ouvert à tous, avec un chat qui ne
demande pas de compte. Quelqu'un qui ne partage aucun serveur avec le bot
ne peut pas lui écrire en privé sur Discord : c'est pour lui que le chat
existe.

En arrière-plan, le bot poste chaque message dans le salon privé
`🤖relais-kumo`, où Kumo lit, et range la réponse dans la conversation.
Les messages privés au bot sur Discord passent par le même relais.

**Le Bureau peut répondre à la place de Kumo** : dans `🤖relais-kumo`,
« Répondre » sur le message relayé envoie la réponse à la personne,
signée de son nom ; ✅ confirme la livraison. Seuls les postes attribués
sur la plateforme comptent, et un message qui ne répond à rien reste dans
le salon.

Contre le spam : 1 000 caractères par message, 8 messages par 10 minutes
par conversation, 100 messages en attente au plus.

### Accès

- **Connexion** : le bouton « Connexion Discord » est remplacé par le logo
  Discord.
- **Ressources** : proposer une ressource demande un compte vérifié.
- **Calendrier** : réservé aux personnes connectées, comme le bandeau
  « À venir » de l'accueil.

**Corrigé** : le test `track_list` attendait encore l'ancien libellé
« Observateur » sans emoji.

## Septembre 2026 (suite) — partages, salons d'annonces, classement réservé

### Partages

Une page **Partages** (📦 dans le menu) où tout membre inscrit dépose un
**fichier** (zip, script, pdf, build… jusqu'à 500 Mo) ou un **lien**
(itch.io, GitHub, Drive…) : un script, du lore, un jeu, des assets. Tous
les membres peuvent télécharger, et chaque partage affiche son **nombre de
téléchargements** — les liens aussi, parce qu'ils passent par la
plateforme. Filtre par type, retrait par l'auteur ou la modération.

Pas d'XP pour un partage : ça récompenserait la quantité, pas la qualité.

Les fichiers sont gardés dans `SHARES_DIR` (par défaut `data/shares`), avec
leur SHA-256. En production, ce dossier doit être sur un volume qui
survit aux redéploiements.

**Corrigé** : télécharger un build dont le nom contient un accent ou un
guillemet cassait la réponse.

### Salons d'annonces

- **`📦partages`** annonce chaque nouveau partage, avec un lien vers la
  page.
- **`📚ressources`** annonce chaque ressource **validée**, avec son lien.
  Les demandes de validation restent dans `🔍a-valider` ou le salon de la
  track : une ressource non validée n'est pas montrée aux membres.

Les deux salons existaient déjà — `🪄créations` et `📰veille-gamedev` —
et ont été renommés plutôt que doublés. Aucun des deux ne retombe sur
`📢annonces` s'il n'est pas configuré.

### Classement réservé aux inscrits

Le classement nomme les membres et leur XP : il n'est plus visible par
quelqu'un qui n'a pas de compte vérifié. Sur la plateforme, la page le
dit, et le lien du menu, le bouton de l'accueil et l'API disparaissent ou
refusent. Sur Discord, `#🏅classement` n'est lisible que par les rôles de
titre membre — le bot remet la permission si on la change à la main — et
`/leaderboard` comme la mention « le classement » répondent de s'inscrire.

## Septembre 2026 (suite) — réunions du Bureau, rail de navigation, bot qui répond

### Réunions du Bureau

Un événement a désormais une **portée** : toute l'association, une track,
ou le Bureau seul. Une réunion de bureau n'apparaît sur le calendrier de
personne d'autre — le filtrage se fait en SQL, parce qu'une ligne qui
arrive dans le navigateur et n'est simplement pas dessinée n'est pas
privée.

L'annonce part dans `🏛bureau`, le salon du Bureau qui existait déjà, en
taguant le rôle Bureau. Un second salon `🏛gc-bureau` avait d'abord été
créé parce que le bot n'avait pas accès au premier ; il a les droits
administrateur désormais, et deux salons Bureau n'avaient pas lieu
d'être. Le journal d'audit y retombe aussi quand aucun salon dédié n'est
configuré, ce qui retire `🏛gc-journal`, resté vide.

### Une interface de jeu

- **Accueil** : un visiteur voit le pitch, les trois gestes du jeu
  (choisir ses tracks, gagner de l'XP, monter en titre), l'échelle des
  titres et les tracks. Un membre connecté voit à la place son panneau de
  mission : ses titres, l'XP avant le prochain, sa série, sa place.
- **Échelle des titres** partout où l'on progresse, avec le titre atteint
  marqué « VOUS » et le dernier titre caché tant que personne ne l'a.
- **Classement** : podium des trois premiers, titre et couleur de chacun.
  Les comptes créés d'avance y figurent avec le titre que vaut leur XP.
- **Profil** : avatar remplacé par l'initiale quand il manque, progression
  vers le prochain titre de chaque track, badges verrouillés ou brillants.
- **Barre du haut** : une jauge vers le prochain titre sous le nom.
- **Quêtes** : récompense mise en avant, tampon « ACCOMPLIE ».
- Polices du design enfin chargées, fond en grille, panneaux et lueurs aux
  couleurs du rang ou de la track, animations désactivées pour qui a
  demandé moins de mouvement.

**Corrigé** : sur téléphone, les pages débordaient en largeur (bouton de
connexion et titre coupés) ; le classement « Saison » était vide parce que
l'XP reportée n'était rattachée à aucune saison ; sur le profil, les
tracks s'écrasaient en colonne étroite.

### Des titres, pas des niveaux

La plateforme n'affiche plus de « Niveau 7 ». Le profil montre la
progression vers le **prochain titre** (« Prochain titre : ⚡ Compagnon de
Guilde — 230 / 400 XP »), la barre du haut le titre actuel, et le
classement n'a plus de colonne niveau. Dans chaque track, on porte un
titre : 👁️ Observateur, 🔨 Contributeur, 🔍 Relecteur, 🧭 Mentor,
⚔️ Co-responsable, 👑 Responsable. `/profil`, `/track` et la réponse
quand on tague le bot montrent les mêmes titres.

### L'XP d'avant l'inscription n'est plus perdue

Toute l'XP passe par `xp::grant` sur la plateforme — GitHub, DraftBot,
présences, quêtes, ajustements du Bureau — et c'est lui qui recalcule
titre, badges et annonces. Mais une montée de niveau DraftBot d'un membre
qui ne s'était pas encore inscrit était **ignorée**. Elle est maintenant
gardée (`draftbot_pending_levels`) et payée à sa première connexion.

### La plateforme décide, Discord suit

Les tracks se choisissent sur la plateforme, les postes du Bureau s'y
attribuent, les titres suivent l'XP. Le bot ne lit plus les rôles Discord
pour les appliquer à la plateforme : il recopie la plateforme sur Discord
(poste, accès `👑✨️Bureau✨️`, titre global, tracks), tout de suite après
chaque changement (`RoleSync` dans l'outbox) et à chaque passe périodique.
Un rôle modifié à la main sur Discord est remis en place.

Les rôles Discord sont rangés du plus important au moins important —
postes du Bureau, titres de membre, tracks — et partout où l'on voit un
membre (profil, barre du haut, `/profil`, `/track`, classement), ses
titres apparaissent dans ce même ordre.

### Report de l'XP Kumo

`scripts/reset-progression.sql` remet la progression à zéro, sauf les
postes du Président et du Vice-président, et reporte l'XP Kumo de chacun
dans son XP globale — le titre de membre, jamais un titre de track.
Un membre qui n'est pas encore inscrit la reçoit à sa première connexion
(`legacy_xp`). Les membres du Bureau ont leur compte créé d'avance (Fred
Vice-président, Kim Secrétaire, et Erwan, Yan, Yann-Ange, crimad sans
poste pour l'instant) ; sur Discord, un compte qui n'a pas encore validé
son email montre déjà le titre que vaut son XP — la validation ouvre les
droits sur la plateforme, pas la reconnaissance. En terminant son inscription, un membre prend directement
le titre que son XP lui vaut, au lieu de repartir d'Initié.

### Poste provisoire du Bureau

`🏛 Membre du Bureau` (`Provisional`) garde l'accès aux salons du Bureau à
un membre dont le poste n'est pas encore décidé, sans rien ouvrir sur la
plateforme : ni panneau d'administration, ni réunions du Bureau. Erwan,
Yan, Yann-Ange et crimad le portent en attendant leur poste.

### Présences

`✅presences` (📌 Informations) reçoit une ligne à chaque présence scannée
par QR, avec l'XP gagnée. Sans repli vers les annonces : une ligne par
scan y noierait tout le reste.

### DraftBot suspendu

La plateforme est le seul système d'XP. Le pont DraftBot → XP est coupé
(`DRAFTBOT_ENABLED=false`), le rôle DraftBot n'a plus aucune permission
et n'a plus accès à `🏆level`. Pour le réactiver : remettre les
permissions du rôle à `396579302911`, retirer l'exception sur `🏆level`,
et `DRAFTBOT_ENABLED=true`.

### Classement et guide tenus par le bot

`🏅classement` et `📖guide` existaient mais restaient vides. Le bot y
tient chacun un message unique, modifié sur place à chaque passe de
synchronisation plutôt que republié.

### Accès Bureau

Les salons du Bureau s'ouvrent au rôle `👑✨️Bureau✨️`, pas aux seize
postes. Un poste attribué sur Discord sans ce rôle donnait des droits sur
la plateforme mais pas l'accès à `🏛bureau` : le bot l'ajoute désormais.
Il ne le retire jamais.

### Plus de catégorie « GAMECLOUD OS »

Tous les salons sont pilotés par la plateforme ; ils ont rejoint les
catégories existantes et perdu le préfixe `gc-` (`🎯quetes`,
`🏅classement` et `📖guide` dans 📌 Informations ; `🏆hall-of-fame` et
`🔍a-valider` dans 🗃 Projet et développement).

### Salons en doublon

`scripts/migrate-legacy-channels.sh` supprime les salons qui en doublent
un autre, `📣gc-annonces`, `🏛gc-bureau` et `🏛gc-journal` compris. Il ne
supprime plus `🏆level` : c'est le salon où le bot lit les montées de
niveau de DraftBot (`DRAFTBOT_CHANNEL_ID`), et le retirer aurait coupé la
synchronisation de l'XP.

### Navigation

La barre horizontale est devenue un **rail vertical groupé** (Moi ·
Association · Responsabilités). Dix destinations sur une ligne finissaient
soit par passer à la ligne, soit par défiler horizontalement sur
téléphone. Le rail devient une barre d'icônes en dessous de 1080 px, puis
une barre basse sur téléphone, où le pouce l'atteint.

### Le bot répond quand on le tague

Il reconnaît quatre intentions — profil, classement, quêtes, agenda — en
tolérant les accents et les formulations approximatives, et répond
directement plutôt que de renvoyer vers un menu. Les réunions du Bureau
sont exclues de sa réponse « agenda » : il répond dans le salon où la
question est posée, qui peut être public.

### Corrigé

- `EventScope::parse` acceptait une track sur une réunion de bureau ;
  elle aurait pu être classée sous une discipline et apparaître dans
  l'agenda de cette track. C'est un test ajouté pour l'occasion qui a
  trouvé l'écart entre le modèle Rust et la contrainte SQL.

---

## Septembre 2026 (suite) — calendrier, présence, notation par track

Les fonctions serveur ne répondaient à aucune route : elles étaient
déclarées sous `/api`, où elles entraient en collision avec l'API REST,
et le gestionnaire `handle_server_fns` n'était jamais enregistré. Aucune
page ne pouvait donc charger quoi que ce soit dans le navigateur. Elles
vivent maintenant sous `/_fn`, qui leur est réservé.

### Ce qui a été ajouté

**Calendrier** (`/calendar`) — séances, ateliers, game jams, réunions,
échéances, présentations. Grille mensuelle qui se replie en agenda sur
téléphone. Les événements s'annulent, ne se suppriment pas : des
présences pointent dessus.

**Présence par QR** — celui qui a les droits génère le code depuis la
page de l'événement ; il est valable pour une durée qu'il choisit
(plafonnée à 24 h). Le code encode un **lien**, pas le jeton brut :
n'importe quel appareil photo l'ouvre, alors que presque aucun ne sait
passer une chaîne décodée à une page web — Firefox et Safari n'ont même
pas `BarcodeDetector`.

**Tableau par track** (`/tracks/{id}`) — ses membres classés par rôle,
les projets qu'elle juge, ses notes, sa moyenne, ses séances.

**Notation** — une note sur 100 accompagne le verdict de chaque track.
Elle est facultative : approuver est une barrière, noter est un
jugement, et un relecteur qui ne veut qu'ouvrir la barrière n'a pas à
inventer un chiffre.

### Ce qui a été corrigé

**Le nom affiché était l'identifiant Discord.** La base ne stockait que
le matricule ; chaque liste, chaque ligne de journal affichait dix-huit
chiffres. Le pseudo et le nom d'affichage sont désormais enregistrés à
la connexion **et** par le bot pour les membres qui ne se sont jamais
connectés.

**Un membre ne revoyait jamais son propre projet.** La liste passait
`false` en dur pour « inclure le non publié », donc elle ne montrait que
les projets publiés — même à l'auteur et au responsable de la track. La
visibilité est maintenant calculée : publié = public, en cours = visible
aux tracks concernées et à l'auteur.

**Créer un projet depuis l'interface ne créait pas de dépôt GitHub.** La
documentation de la fonction le promettait, mais seule la route REST
provisionnait. La logique est partagée ; les deux chemins créent le
dépôt.

**Un membre pouvait être payé deux fois pour une séance** si
l'organisateur réimprimait le code : chaque impression est un jeton
distinct. Un index unique sur `(user_id, event_id)` le rend impossible.

**`events::create` ne liait jamais son `created_by`.** L'insertion aurait
échoué à la première utilisation.

---

## Septembre 2026 — audit, corrections, et fin de la plateforme

Cette version part d'un audit complet du code et corrige ce qu'il a
trouvé, puis termine les fonctionnalités qui n'existaient qu'en base de
données.

**Résumé :** 27 → **149 tests**, 13 → **46 routes**, 8 → **20 tables
réellement utilisées**, 2 pages placeholder → **9 pages réelles**.

---

## 🔐 Sécurité

### Secrets factices acceptés au démarrage

`JWT_SECRET`, `DRAFTBOT_API_KEY` et `GITHUB_WEBHOOK_SECRET` étaient
encore les valeurs `change-me…` de `.env.example`. La seule vérification
était une longueur ≥ 32 octets, qu'un placeholder passe sans problème.
Un `JWT_SECRET` devinable permet de forger un token d'accès pour
n'importe quel UUID — c'est-à-dire n'importe quel compte.

**Correction** : `Config::from_env` refuse désormais de démarrer si un
secret est trop court **ou** commence par l'un des préfixes documentés
(`change`, `your-`, `replace`, …).

> ⚠️ **Le `.env` réel était inclus dans `gamecloud-os.zip`**, avec un
> token de bot Discord et un client secret d'apparence valide. S'il a été
> partagé, il faut les régénérer dans le portail Discord Developer.

### Force brute sur l'OTP

`verify_otp` bloquait après 5 tentatives, mais `upsert_otp` supprimait la
ligne et la réinsérait avec `attempts = 0`. Il suffisait de redemander un
code pour récupérer 5 essais — indéfiniment, sur un secret à 6 chiffres.

**Correction** : un compteur `cumulative_attempts` qui survit aux
renvois, un délai minimum entre deux demandes (60 s), et un plafond de
renvois (5). L'adresse est aussi vérifiée comme libre **avant** l'envoi
du code, pour que l'endpoint ne serve pas à spammer une boîte mail.

### Aucune limite de débit

`ARCHITECTURE.md` annonçait une couche Tower de rate limiting depuis le
début ; `middleware/` ne contenait que `auth` et `github_signature`.

**Correction** : `middleware/rate_limit.rs`, une fenêtre fixe par IP,
plus stricte sur les endpoints sensibles (OTP, login, scan, sync).

### Autres

- **Changement d'email après vérification** — `Action::SubmitEpitechEmail`
  exige `rank == Pending`, mais le handler ne consultait jamais la
  permission. Un membre vérifié pouvait revérifier une autre adresse.
- **CORS permissif** sur une API authentifiée par cookie → restreint à
  `PUBLIC_ORIGIN`.
- **Tokens QR en clair en base** → hachés en SHA-256, comme les refresh
  tokens l'étaient déjà.
- **`/api/sync/draftbot`** parsait le corps JSON *avant* de vérifier la
  clé d'API, et faisait confiance à `new_level` (jusqu'à 9999 → 49 995
  XP). Le corps n'est plus lu avant authentification, le niveau est
  plafonné, et un même niveau n'est payé qu'une fois.

---

## 🐛 Correction de logique

### Le QR de présence n'était valable que pour une personne

`claim_qr_token` posait `is_used = TRUE` au premier scan. À une session
du lundi, le premier membre à scanner prenait l'XP et les autres
recevaient `410`. La fonctionnalité ne pouvait pas faire ce pour quoi
elle existe.

**Correction** : le token reste valable jusqu'à son expiration (ou une
capacité optionnelle), et un index unique sur `(user_id, qr_token_id)`
empêche un même membre de le réclamer deux fois.

### Le plafond quotidien ne plafonnait pas

`handle_push` coupait la valeur brute à 50, **puis** appliquait les
multiplicateurs (jusqu'à ×2,6) — soit 130 XP pour un plafond de 50. Le
compteur additionnait par ailleurs tout l'XP `GitHub` du jour, donc une
PR mergée mangeait le budget commits pendant que l'XP de PR n'était,
elle, jamais plafonnée.

**Correction** : `apply_daily_cap` s'applique **après** les
multiplicateurs, et le plafond commits ne compte que les lignes commits.

### Deux chemins d'attribution d'XP divergents

Le webhook appelait `grant_xp` ; le scan QR réécrivait sa propre
insertion et ne recalculait jamais le rang. L'XP de présence ne
promouvait donc personne.

**Correction** : un seul moteur, `db::queries::xp::grant`, qui porte
toute la cascade (multiplicateurs → plafond → ledger → totaux → rang →
niveau → série → track → quêtes → badges → annonces), en une
transaction.

### Le rang sautait `Initiate`

Le `CASE` SQL faisait passer un `Visitor` directement à `Apprentice` à
150 XP. Le moteur délègue maintenant à `GlobalRank::from_xp`, l'unique
source de vérité annoncée par la doc du crate partagé.

---

## ⚡ Mécaniques branchées mais mortes

Trois systèmes étaient entièrement câblés et n'avaient jamais fonctionné.

| Mécanique | Pourquoi elle ne marchait pas | Correction |
|---|---|---|
| **Séries (streaks)** | `streak_days` n'était jamais écrit, donc le multiplicateur valait toujours 1.0 | Calcul et écriture à chaque gain d'XP, avec `longest_streak` |
| **Tracks** | Rien n'insérait dans `track_memberships` : pas d'XP de track, pas de rôles de track, bonus multi-track figé à 1.0 | Étape 3 de l'onboarding + `/api/users/me/tracks` |
| **Annonces Discord** | Le bot interrogeait `notifications_outbox` — que personne ne remplissait | `services/notifications.rs`, écrit dans la transaction de l'événement |

---

## 🆕 Nouveau

- **Saisons** — le classement vit est borné à la saison ouverte. Les
  rangs, niveaux et badges restent cumulatifs.
- **Moteur de quêtes** — progression automatique depuis les événements
  d'XP, complétion idempotente, paiement sans multiplicateur.
- **Moteur de badges** — 11 badges automatiques réévalués après chaque
  gain ; 4 badges restent à la main du Bureau.
- **Pipeline projets** — cycle de vie complet, validation par track avec
  retour écrit obligatoire sur un refus, rareté dérivée du nombre de
  tracks satisfaites, Hall of Fame et paiement de toute l'équipe.
- **Bibliothèque de ressources** — soumission, validation, un vote par
  membre.
- **Journal d'audit** — `audit_logs` a enfin un auteur d'écriture.
- **API membres et admin** — profil, historique, tracks, badges, quêtes,
  classements, feuille de personnage, panneau Bureau.
- **Rôles Discord** — les rangs sont reflétés sur de vrais rôles du
  serveur (opt-in via `DISCORD_GUILD_ID`).

### Frontend

`/projects` et `/leaderboard` n'étaient plus des placeholders : vraies
pages, plus `/quests`, `/scan`, `/projects/:id`, `/onboarding/tracks`, et
un profil transformé en feuille de personnage (niveau, série, tracks,
badges, ledger).

---

## 🧹 Qualité

- Suppression des quatre « ancres d'import » (`let _ = (…)`,
  `_STATUS_ANCHOR`, `keep import alive`).
- `mailer.rs` : nettoyage du patch manuel (`// ← this line was missing`).
- `GlobalRank` / `BureauRole` / `SpecialBadge` ont désormais `parse` et
  `as_str` dans le crate partagé ; les quatre copies locales de ces
  `match` ont disparu.
- `Option<CurrentUser>` fonctionne — l'extracteur que la doc du module
  promettait depuis le début n'existait pas.
- `SELECT *` remplacé par des listes de colonnes explicites.
- **Le dépôt est enfin sous git.**

---

## 📈 Évolution

| Métrique | Avant | Après |
|---|---:|---:|
| Tests | 27 | **149** |
| Routes HTTP | 13 | **46** |
| Tables utilisées / migrées | 8 / 20 | **20 / 23** |
| Pages réelles | 4 (+2 placeholders) | **9** |
| Commandes Discord | 4 | **6** |
| Multiplicateurs fonctionnels | 0 / 2 | **2 / 2** |
| `clippy::pedantic -D warnings` | passe | **passe** |

---

# Diagnostic et corrections — Mai 2026

Ce qui suit est le journal de la campagne précédente, conservé pour
mémoire.

## 🐛 Bugs critiques (bloquants)

### Bug #1 — Ton email Epitech était rejeté

**Symptôme** : impossible de soumettre `joachim.goeh-akue@epitech.eu`
à `/onboarding/email`. La regex côté serveur ET la contrainte CHECK
en base de données rejetaient les **hyphens** dans le nom.

**Fix** : regex `^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*@epitech\.eu$` et
migration `0008_email_regex_relax.sql`.

### Bug #2 — « Obligé de refresh avant d'être redirigé sur Discord »

Le routeur Leptos interceptait les `<a>` internes après hydratation.
**Fix** : `rel="external"` sur les liens qui doivent déclencher une
vraie navigation HTTP.

### Bug #3 — Les formulaires d'onboarding renvoyaient une erreur

Les handlers extrayaient un `Json<…>` alors que les `<form>` envoient
du `x-www-form-urlencoded`. **Fix** : négociation de contenu.

### Bug #4 — `/profile` affichait « Pas connecté » même connecté

La prop `me=None` était codée en dur. **Fix** : server function Leptos
lisant le cookie `gc_access`.

## 🐞 Bugs UX

- **#5** Redirection post-OAuth intelligente (`post_login_target`).
- **#6** `/api/auth/logout` accepte GET et POST.
- **#7** Pages `/projects` et `/leaderboard` créées (placeholders).
- **#8** État de connexion visible dans le HUD.
- **#9** `SUPABASE_*` et `GITHUB_WEBHOOK_SECRET` rendus optionnels.
