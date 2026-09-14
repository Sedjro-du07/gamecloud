# Diagnostic et corrections — Mai 2026

Ce fichier liste **tous les bugs trouvés dans la version que tu as
testée**, et **comment chacun a été corrigé**. Lis-le pour comprendre
pourquoi telle chose ne marchait pas.

---

## 🐛 Bugs critiques (bloquants)

### Bug #1 — Ton email Epitech était rejeté

**Symptôme** : impossible de soumettre `joachim.goeh-akue@epitech.eu`
à `/onboarding/email`. La regex côté serveur ET la contrainte CHECK
en base de données rejetaient les **hyphens** dans le nom.

**Cause** : la regex `^[a-z]+\.[a-z]+@epitech\.eu$` n'acceptait que des
lettres minuscules pures.

**Fix** :
- `crates/web/src/services/email_validator.rs` — nouvelle regex
  `^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*@epitech\.eu$` (autorise les hyphens
  et chiffres dans le nom, mais exige une lettre en première position).
- `migrations/0008_email_regex_relax.sql` — nouvelle migration qui
  relâche les contraintes CHECK sur `users.email` et `email_otps.email`
  pour matcher la regex applicative.
- 3 nouveaux tests unitaires :
  - `accepts_hyphenated_names` (ton cas)
  - `accepts_digits_in_names`
  - `rejects_leading_digits`

---

### Bug #2 — "Obligé de refresh avant d'être redirigé sur Discord"

**Symptôme** : le premier clic sur **"Se connecter avec Discord"** ne
faisait rien ; il fallait recharger la page et **re-cliquer** pour que
ça parte vers Discord.

**Cause** : le routeur Leptos client-side intercepte tous les `<a>`
internes après hydratation. Comme `/api/auth/login` n'est pas une
route déclarée dans `<Routes>`, le routeur tentait de naviguer mais ne
trouvait rien à rendre, et le navigateur ne faisait pas une vraie
requête HTTP.

**Fix** : ajout de `rel="external"` sur tous les liens `<a>` qui doivent
déclencher une navigation HTTP réelle (vers une route Axum, pas Leptos).
Leptos respecte cet attribut et laisse le navigateur faire le travail.

Fichiers touchés :
- `crates/web/src/pages/home.rs` (CTA de connexion)
- `crates/web/src/pages/profile.rs` (lien de connexion + onboarding)
- `crates/web/src/pages/onboarding.rs` (liens entre étapes)
- `crates/web/src/components/hud_shell.rs` (boutons login/logout dans la nav)

---

### Bug #3 — Les formulaires d'onboarding renvoyaient une erreur

**Symptôme** : impossible de soumettre l'email Epitech ou le code OTP.
Le serveur répondait `415 Unsupported Media Type` ou `422 Unprocessable
Entity`.

**Cause** : les pages `/onboarding/email` et `/onboarding/verify`
utilisent un `<form method="post">` HTML standard, qui envoie
`application/x-www-form-urlencoded`. Mais les handlers `submit_email`
et `verify_otp` extrayaient un `Json<...>`, ce qui échoue sans
content-type matching.

**Fix** : refactoring des deux handlers pour accepter **les deux
formats** :
- détection du `Content-Type` du request
- décodage via `serde_urlencoded` ou `serde_json` selon le cas
- en cas de succès sur form, redirection 303 vers l'étape suivante
  (au lieu d'un JSON ack que le navigateur afficherait brut)

Fichier : `crates/web/src/routes/auth.rs` — fonctions `submit_email` et
`verify_otp` complètement refaites.

---

### Bug #4 — `/profile` affichait "Pas connecté" même quand connecté

**Symptôme** : après avoir fini l'OTP, la page profil disait toujours
"Pas encore connecté". Aucun moyen de voir sa carte de personnage.

**Cause** : la déclaration de route était :
```rust
<Route path="profile" view=|| view! { <ProfilePage me=None /> } />
```
La prop `me=None` était hardcodée. Le composant ne tentait jamais de
fetch.

**Fix** : ajout d'une **server function** Leptos qui lit le cookie
`gc_access` côté serveur et retourne le profil :

```rust
#[server(GetCurrentUser, "/api")]
pub async fn get_current_user() -> Result<Option<CurrentUserView>, ServerFnError>
```

Le composant `ProfilePage` utilise un `Resource` pour appeler cette
server fn, et un `<Suspense>` pour afficher un état de chargement,
puis la carte avec les vraies données.

---

## 🐞 Bugs UX (non-bloquants)

### Bug #5 — Pas de redirection intelligente après OAuth

**Symptôme** : après l'OAuth, tu étais toujours envoyé vers
`/onboarding/email`, même si ton compte était déjà entièrement vérifié.

**Fix** : nouvelle fonction `post_login_target` dans `auth.rs` qui
inspecte `record.email_verified` et `record.email`, et redirige vers :
- `/profile` si déjà vérifié
- `/onboarding/verify` si email soumis mais pas vérifié
- `/onboarding/email` sinon (compte `Pending`)

---

### Bug #6 — Bouton "Déconnexion" ne marchait pas

**Symptôme** : un lien `<a href="/api/auth/logout">` dans la nav devait
déconnecter, mais le handler n'acceptait que POST.

**Fix** : la route `/api/auth/logout` accepte maintenant **GET et POST**
(les deux sont idempotents). Le bouton de la nav peut donc être un
simple lien.

---

### Bug #7 — Liens `/projects` et `/leaderboard` → 404

**Symptôme** : la nav contenait ces liens mais aucune route ne leur
correspondait.

**Fix** : création de deux pages stub :
- `crates/web/src/pages/projects.rs` — message "à venir"
- `crates/web/src/pages/leaderboard.rs` — pointe vers la commande
  Discord `/leaderboard` qui marche déjà

Les routes sont maintenant déclarées dans `app.rs`.

---

### Bug #8 — HUD ne reflétait pas l'état de connexion

**Symptôme** : pas moyen de savoir d'un coup d'œil si on était connecté
ou non.

**Fix** : ajout d'un slot d'auth dans le HudShell :
- `<span class="gc-pill">{username}</span>` + lien "Déconnexion" si
  connecté
- `<a class="gc-btn">Connexion Discord</a>` sinon

Utilise la même server fn `get_current_user` que la page profil.

---

### Bug #9 — `SUPABASE_*` était requis pour démarrer

**Symptôme** : le serveur refusait de démarrer si `SUPABASE_URL` ou
`SUPABASE_SERVICE_KEY` étaient absents, alors qu'ils ne sont pas
encore utilisés en phase 4.

**Fix** : `Config::supabase_url` et `supabase_service_key` sont
maintenant `Option<String>`. Idem pour `github_webhook_secret`. Le
handler du webhook GitHub renvoie `503 Service Unavailable` quand le
secret est absent (au lieu de crasher au boot).

---

## 📈 Évolution

| Métrique | Avant | Après |
|----------|------:|------:|
| Tests passants | 24 | **27** (3 nouveaux sur l'email) |
| Bugs bloquants | 4 | **0** |
| Bugs UX | 5 | **0** |
| `cargo clippy -- -D warnings` | passe | **passe** |
| Format email accepté | strict | **réel** (hyphens + digits) |
| Page profil fonctionnelle | non | **oui** (avec server fn) |
| Démarrage sans Supabase | non | **oui** |

---

## 🆕 Nouveau dans ce zip

- `QUICKSTART.md` — guide pas-à-pas pour démarrer en local
- `CHANGELOG_FIX.md` — ce fichier
- `migrations/0008_email_regex_relax.sql` — relâche les contraintes DB
- `crates/web/src/pages/projects.rs` — stub /projects
- `crates/web/src/pages/leaderboard.rs` — stub /leaderboard

## 📝 Pour tester

1. `cp .env.example .env` puis remplis (voir `QUICKSTART.md`)
2. `createdb gamecloud_dev`
3. `cargo leptos watch`
4. Ouvre http://localhost:3000 et clique sur **Connexion Discord**
   — la redirection est immédiate cette fois
5. Soumets ton email `joachim.goeh-akue@epitech.eu` — accepté
6. Tape l'OTP reçu — tu arrives sur `/profile` qui montre ta carte
