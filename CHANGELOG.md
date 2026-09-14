# Changelog

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
