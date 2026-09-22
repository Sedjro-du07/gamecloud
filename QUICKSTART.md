# QUICKSTART — GameCloud OS

Guide pas-à-pas pour avoir une plateforme **fonctionnelle en local** en
~15 minutes. Suis les étapes dans l'ordre.

---

## 1. Pré-requis

- Rust stable (≥ 1.80) — `rustup default stable`
- PostgreSQL 14+ (local, Docker, ou Supabase)
- Un compte Discord Developer (gratuit)

---

## 2. Installer les outils Cargo

```bash
cargo install --locked cargo-leptos
cargo install --locked sqlx-cli --no-default-features --features postgres,rustls
rustup target add wasm32-unknown-unknown
```

---

## 2 bis. Générer les secrets — obligatoire

Le serveur **refuse de démarrer** tant que `JWT_SECRET`,
`DRAFTBOT_API_KEY` et `GITHUB_WEBHOOK_SECRET` valent encore les
placeholders de `.env.example`. Ce n'est pas une chicane : un
`JWT_SECRET` devinable permet de forger un token d'accès pour n'importe
quel compte.

```bash
for var in JWT_SECRET DRAFTBOT_API_KEY GITHUB_WEBHOOK_SECRET; do
  echo "$var=$(openssl rand -base64 48 | tr -d '\n')"
done
```

Colle les trois lignes dans ton `.env` (en remplaçant celles qui y sont).
`DRAFTBOT_API_KEY` doit être **identique** pour le web et le bot : les
deux lisent le même `.env` en développement.

---

## 3. Discord — créer ton application OAuth + bot

1. Va sur https://discord.com/developers/applications → **New Application**
2. Nomme-la "GameCloud OS (dev)"
3. Dans **OAuth2 → General** :
   - Note `CLIENT ID` → `DISCORD_CLIENT_ID`
   - Reset Secret → note la valeur → `DISCORD_CLIENT_SECRET`
   - Sous **Redirects**, ajoute : `http://localhost:3000/api/auth/callback`
4. Dans **Bot** :
   - **Add Bot**
   - Reset Token → note la valeur → `DISCORD_TOKEN`
   - Active **MESSAGE CONTENT INTENT** et **SERVER MEMBERS INTENT**
5. Dans **OAuth2 → URL Generator**, coche les scopes `bot` et `applications.commands`,
   puis ouvre l'URL pour inviter le bot sur ton serveur de test.

---

## 4. Préparer la base de données

### Option A — PostgreSQL local

```bash
createdb gamecloud_dev
# Optionnel : créer un user dédié
psql postgres -c "CREATE USER gamecloud WITH PASSWORD 'gamecloud';"
psql postgres -c "GRANT ALL PRIVILEGES ON DATABASE gamecloud_dev TO gamecloud;"
```

DSN : `postgres://gamecloud:gamecloud@localhost:5432/gamecloud_dev`

### Option B — Docker

```bash
docker run -d --name gc-pg \
  -e POSTGRES_USER=gamecloud \
  -e POSTGRES_PASSWORD=gamecloud \
  -e POSTGRES_DB=gamecloud_dev \
  -p 5432:5432 postgres:16
```

### Option C — Supabase

Va sur supabase.com, crée un projet, copie le **Connection string**
en mode "Transaction Mode" pour le DSN.

---

## 5. Pas de mail

La plateforme n'envoie aucun mail et ne demande aucune adresse : se
connecter avec Discord suffit. Il n'y a rien à configurer ici.

---

## 6. Configurer `.env`

```bash
cp .env.example .env
```

Édite `.env` et remplis chaque champ. Les variables critiques :

```ini
APP_ENV=dev
PUBLIC_ORIGIN=http://localhost:3000
BIND_ADDR=0.0.0.0:3000
DATABASE_URL=postgres://gamecloud:gamecloud@localhost:5432/gamecloud_dev

# Génère avec: openssl rand -base64 48
JWT_SECRET=<au moins 32 caractères aléatoires>

DISCORD_CLIENT_ID=<de l'étape 3>
DISCORD_CLIENT_SECRET=<de l'étape 3>
DISCORD_REDIRECT_URI=http://localhost:3000/api/auth/callback

DISCORD_TOKEN=<de l'étape 3>
DRAFTBOT_CHANNEL_ID=<ID du channel où DraftBot post (clic droit → Copy Channel ID, mode dev activé)>
DRAFTBOT_USER_ID=<ID utilisateur de DraftBot>
GAMECLOUD_SYNC_URL=http://localhost:3000
DRAFTBOT_API_KEY=<une chaîne random partagée web/bot>

GITHUB_WEBHOOK_SECRET=<random pour plus tard>


# Optionnels pour démarrer (laisse vide tant que tu n'en as pas besoin)
SUPABASE_URL=
SUPABASE_SERVICE_KEY=
GITHUB_WEBHOOK_SECRET=
```

> ✅ **`SUPABASE_*` et `GITHUB_WEBHOOK_SECRET` sont maintenant optionnels** :
> tu peux les laisser vides pour démarrer, le serveur tourne quand même.

---

## 7. Lancer le web

Depuis la racine du repo :

```bash
cargo leptos watch
```

Au premier lancement :
1. Compile le serveur (~5 min)
2. Compile le bundle WASM (~3 min)
3. Applique les 8 migrations sur ta DB
4. Démarre sur http://localhost:3000

Si tu vois `gamecloud-web listening` dans les logs JSON, c'est bon.

---

## 8. Tester le flow OAuth Discord

1. Ouvre http://localhost:3000
2. Clique **"Se connecter avec Discord"**
3. Tu es redirigé vers Discord, autorises l'app
4. Discord redirige vers `/api/auth/callback?code=...&state=...`
5. Le serveur :
   - vérifie le state (cookie CSRF)
   - échange le code contre un access_token
   - récupère ton ID Discord
   - upsert un row `users` avec `global_rank = 'Initiate'`
   - pose `gc_access` (JWT) et `gc_refresh` (token opaque) cookies
   - **redirige vers `/profile`** — pas d'étape de mail

Si ça échoue à cette étape, regarde les logs : ils sont en JSON
structuré et précis sur la cause (state mismatch, Discord token
exchange failed, etc.).

---

## 9. Après la connexion

Discord te renvoie directement sur `/profile` : il n'y a pas d'étape
d'adresse mail. Si tu es sur le serveur Discord de l'association, tu es
membre ; sinon la plateforme t'oriente vers le test d'entrée.

Tu vois ton avatar Discord, ton titre **L'Initié** (ou celui que ton XP
vaut déjà) et ta barre vers le titre suivant.

---

## 10. Lancer le bot Discord (optionnel pour ce stade)

Dans un autre terminal :

```bash
cargo run -p gamecloud-bot
```

Le bot enregistre les slash commands globalement (peut prendre jusqu'à
1h pour qu'elles apparaissent côté Discord ; en dev, force-les à
apparaître en tapant `/` dans Discord, plusieurs fois).

Test : tape `/profil` dans n'importe quel channel du serveur → tu
vois ta carte.

---

## 11. Vérifier la DB

```bash
psql $DATABASE_URL -c "SELECT id, discord_id, email, global_rank, xp_total FROM users;"
```

Tu dois voir ta row.

---

## 🐛 Dépannage rapide

| Symptôme | Cause probable | Fix |
|----------|----------------|-----|
| "Obligé de refresh avant Discord" | Lien intercepté par le router Leptos | Corrigé via `rel="external"` |
| Form onboarding renvoie 415/422 | Body en JSON attendu | Corrigé : handlers acceptent form-urlencoded |
| Email valide rejeté | Regex trop stricte | Corrigé : hyphens et chiffres autorisés |
| Profile montre "Pas connecté" alors que je le suis | Server fn ne lit pas le cookie | Corrigé via `use_context::<Parts>()` |
| `MissingVar(...)` au démarrage | Variable required vide | Voir `.env.example` colonne **REQUIRED** |
| Bot ne réagit pas aux level-ups DraftBot | `DRAFTBOT_CHANNEL_ID` ou `DRAFTBOT_USER_ID` faux | Active mode dev Discord, click-droit → Copy ID |
| Migration `0008` ne s'applique pas | Une migration plus ancienne a échoué | `psql … -c "DROP TABLE _sqlx_migrations;"` puis relancer |

---

## ✅ Checklist post-installation

- [ ] `cargo leptos watch` démarre sans crash
- [ ] http://localhost:3000 affiche la home avec le titre Cyberpunk
- [ ] Click "Connexion Discord" me redirige vers Discord (sans refresh)
- [ ] Après autorisation, j'arrive directement sur `/profile`, cookies posés
- [ ] `/profile` affiche ma carte avec mon avatar Discord et ma XP
- [ ] `cargo run -p gamecloud-bot` démarre, `/profil` Discord répond

Si toutes ces cases sont cochées, ta plateforme tourne.

---

## 8. Le parcours complet, à tester

Une fois `cargo leptos watch` et le bot lancés :

1. **http://localhost:3000** → « Connexion Discord ». La redirection est
   immédiate.
2. Tu arrives directement sur ton profil : aucune étape de mail.
3. **`/onboarding/tracks`** → choisis une track, et une spécialisation si
   tu veux. Ton XP de track commence à compter. Tant qu'elle n'est pas faite, un bandeau te le
   rappelle sur toutes les pages.
4. **`/profile`** → ta feuille de personnage : niveau, série, tracks,
   badges, historique d'XP.
5. **`/leaderboard`** → classement de la saison, par défaut. Les onglets
   « Depuis toujours » et « Par track » sont là aussi.
6. **`/quests`** → vide au départ. Crée-en une avec
   `POST /api/quests` (il faut un rôle Bureau exécutif).
7. **`/scan`** → colle un token produit par `POST /api/qr/generate`.
   Plusieurs membres peuvent scanner le même code ; chacun une seule
   fois.

### Se donner un rôle Bureau en local

Le panneau admin et la création de quêtes demandent un rôle exécutif.
En développement, le plus simple est de se l'attribuer en SQL :

```bash
psql gamecloud_dev -c "UPDATE users SET offices = '{President}' WHERE discord_username = 'ton_pseudo_discord';"
```

### Annonces Discord

Pour voir les montées de rang et les badges s'afficher dans Discord,
renseigne `DISCORD_ANNOUNCE_CHANNEL_ID` (clic droit sur un salon →
« Copier l'identifiant », mode développeur activé). Sans cette variable,
la plateforme fonctionne normalement mais reste silencieuse.

### Rôles de rang Discord

Renseigne `DISCORD_GUILD_ID` et crée dans ton serveur un rôle par rang,
nommé exactement comme le titre du rang (`🌱 L'Initié`, etc.). Place le
rôle du bot **au-dessus** de ces rôles. Voir `docs/DEPLOYMENT.md`.
