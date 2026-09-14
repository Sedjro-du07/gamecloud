# GameCloud OS — Deployment

Reference for shipping `gamecloud-web` and `gamecloud-bot` to a single
Linux host fronted by Caddy or nginx, against a managed PostgreSQL.

## Targets

| Component | Native binary | Memory (steady) | CPU (steady) |
|-----------|---------------|-----------------|--------------|
| `gamecloud-web` | `target/release/gamecloud-web` | ~80 MB | ~5% |
| `gamecloud-bot` | `target/release/gamecloud-bot` | ~40 MB | ~2% |
| PostgreSQL | managed | — | — |
| Supabase Storage | managed | — | — |

## Environment variables

The two binaries share `DATABASE_URL` and `DRAFTBOT_API_KEY`; otherwise
their environments are disjoint.

### `gamecloud-web`

| Variable | Required | Default | Notes |
|----------|----------|---------|-------|
| `BIND_ADDR` | no | `0.0.0.0:3000` | TCP listen address |
| `PUBLIC_ORIGIN` | yes | — | Used in OAuth redirects, cookies, emails |
| `APP_ENV` | no | `dev` | `production` enables `Secure` cookies |
| `DATABASE_URL` | yes | — | `postgres://user:pass@host:5432/db` |
| `JWT_SECRET` | yes | — | ≥ 32 bytes, random |
| `JWT_ACCESS_TTL` | no | `3600` | seconds |
| `JWT_REFRESH_TTL` | no | `604800` | seconds (7d) |
| `JWT_QR_TTL` | no | `7200` | seconds |
| `DISCORD_CLIENT_ID` | yes | — | Discord OAuth |
| `DISCORD_CLIENT_SECRET` | yes | — | Discord OAuth |
| `DISCORD_REDIRECT_URI` | yes | — | e.g. `https://gc.example/api/auth/callback` |
| `GITHUB_WEBHOOK_SECRET` | yes | — | HMAC secret for GitHub webhooks |
| `DRAFTBOT_API_KEY` | yes | — | Shared with the bot |
| `SMTP_HOST` | yes | — | e.g. `smtp.eu.mailgun.org` |
| `SMTP_PORT` | no | `587` | STARTTLS port |
| `SMTP_USERNAME` | yes | — | |
| `SMTP_PASSWORD` | yes | — | secret manager recommended |
| `SMTP_FROM` | yes | — | e.g. `GameCloud <noreply@gc.example>` |
| `SUPABASE_URL` | yes | — | `https://<id>.supabase.co` |
| `SUPABASE_SERVICE_KEY` | yes | — | server-side only |

Source: `crates/web/src/config.rs`.

### `gamecloud-bot`

| Variable | Required | Default | Notes |
|----------|----------|---------|-------|
| `DISCORD_TOKEN` | yes | — | bot token |
| `DATABASE_URL` | yes | — | same DSN as web |
| `DRAFTBOT_CHANNEL_ID` | yes | — | Discord channel where DraftBot posts |
| `DRAFTBOT_USER_ID` | yes | — | DraftBot's user ID |
| `GAMECLOUD_SYNC_URL` | yes | — | base URL of `gamecloud-web` |
| `DRAFTBOT_API_KEY` | yes | — | shared with web |
| `OUTBOX_POLL_SECONDS` | no | `5` | notifications poll interval |

Source: `crates/bot/src/config.rs`.

## Build

```bash
# 1. Install cargo-leptos (one-time)
cargo install --locked cargo-leptos

# 2. Build the web binary + WASM bundle (release)
cargo leptos build --release

# 3. Build the bot binary
cargo build --release -p gamecloud-bot
```

After step 2 you have:

- `target/release/gamecloud-web` — Axum + Leptos SSR binary
- `target/site/` — static assets (`pkg/gamecloud.{js,wasm,css}`,
  fonts, etc.); the binary serves these directly

After step 3 you have `target/release/gamecloud-bot`.

## Database setup

```bash
# Create the database
createdb gamecloud

# Apply all migrations (run once at deploy and on every release)
sqlx database setup        # if you use sqlx-cli
# or just let the web binary do it on startup — it calls
# sqlx::migrate!() on every boot
```

The web binary runs migrations on startup (see
`crates/web/src/db/pool.rs::migrate`). It is safe to call repeatedly:
`_sqlx_migrations` tracks what has run.

## systemd units

```ini
# /etc/systemd/system/gamecloud-web.service
[Unit]
Description=GameCloud OS web
After=network.target postgresql.service

[Service]
Type=simple
User=gamecloud
Group=gamecloud
WorkingDirectory=/opt/gamecloud
EnvironmentFile=/etc/gamecloud/web.env
ExecStart=/opt/gamecloud/bin/gamecloud-web
Restart=always
RestartSec=5
LimitNOFILE=65536
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/opt/gamecloud/site
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/gamecloud-bot.service
[Unit]
Description=GameCloud OS Discord bot
After=network.target postgresql.service gamecloud-web.service
Wants=gamecloud-web.service

[Service]
Type=simple
User=gamecloud
Group=gamecloud
WorkingDirectory=/opt/gamecloud
EnvironmentFile=/etc/gamecloud/bot.env
ExecStart=/opt/gamecloud/bin/gamecloud-bot
Restart=always
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

## Caddy reverse proxy

```caddy
gc.example.com {
    encode zstd gzip

    handle /api/webhooks/* {
        # GitHub needs the raw body for HMAC; don't buffer.
        reverse_proxy localhost:3000 {
            transport http {
                read_buffer  64KB
                write_buffer 64KB
            }
        }
    }

    handle /pkg/* {
        # Long cache for the immutable WASM bundle.
        header Cache-Control "public, max-age=31536000, immutable"
        reverse_proxy localhost:3000
    }

    handle {
        reverse_proxy localhost:3000
    }
}
```

## Health checks

| Probe | URL | Frequency | Action on failure |
|-------|-----|-----------|-------------------|
| Liveness | `GET /healthz` | 10 s | restart |
| Readiness | `GET /readyz` | 30 s | drain from LB |

`readyz` returns 503 if the database pool can't run `SELECT 1`.

## Secret rotation

| Secret | Rotation |
|--------|----------|
| `JWT_SECRET` | Rotate by writing the new value, restarting; in-flight access tokens become invalid (1h max). |
| `DISCORD_CLIENT_SECRET` | Regenerate in the Discord Developer Portal, swap, restart. |
| `GITHUB_WEBHOOK_SECRET` | Update in GitHub then in the env, swap atomically. |
| `DRAFTBOT_API_KEY` | Update both `web.env` and `bot.env`, restart bot first then web. |
| Refresh tokens | Auto-rotated on every `/api/auth/refresh`; reuse triggers global revocation. |

## Backups

- PostgreSQL: managed PITR at the cloud level. Logical dumps nightly:
  `pg_dump --format=c gamecloud > /backup/$(date -I).pgc`.
- Supabase Storage: covered by Supabase's own retention.
- Secrets: store in a vault, never in version control. The `.env`
  files referenced above are produced from the vault at deploy time.

## First-time bootstrap checklist

1. Provision the host, install PostgreSQL or point at a managed DB.
2. Create the OS user `gamecloud`, the dirs `/opt/gamecloud`,
   `/etc/gamecloud`.
3. Drop the two binaries in `/opt/gamecloud/bin/` and the static
   site folder in `/opt/gamecloud/site/`.
4. Write `/etc/gamecloud/web.env` and `bot.env`.
5. Run the web binary once manually to apply migrations and verify it
   starts; check `/healthz`.
6. Insert the founding member into the DB by hand:
   ```sql
   UPDATE users
       SET bureau_role = 'President', global_rank = 'Legend'
       WHERE discord_id = '<your discord id>';
   ```
7. Enable both systemd units, configure Caddy, register the GitHub
   webhook with the secret.

---

## Environment variables added in September 2026

### Secrets are validated at boot

`gamecloud-web` **refuses to start** if `JWT_SECRET`, `DRAFTBOT_API_KEY`
or `GITHUB_WEBHOOK_SECRET` is shorter than 32 bytes, or still begins with
one of the placeholder prefixes shipped in `.env.example` (`change`,
`CHANGE`, `your-`, `YOUR_`, `replace`, `xxxxx`).

This is deliberate. A guessable `JWT_SECRET` lets anyone forge an access
token for any member UUID, which is total account takeover; failing to
boot is a far better outcome than running in that state. Generate each
secret with:

```bash
openssl rand -base64 48
```

### New variables

| Variable | Binary | Default | Meaning |
|---|---|---|---|
| `DISCORD_ANNOUNCE_CHANNEL_ID` | web | *(unset)* | Channel that receives rank-ups, badges, quest completions and releases. Unset ⇒ the platform stays silent; outbox rows are not written. |
| `RATE_LIMIT_SENSITIVE_PER_MIN` | web | `10` | Per-IP budget for OTP, login, QR scan, DraftBot sync. |
| `RATE_LIMIT_DEFAULT_PER_MIN` | web | `120` | Per-IP budget for everything else. |
| `TRUST_FORWARDED_FOR` | web | `false` | Whether `X-Forwarded-For` identifies the client. |
| `DISCORD_GUILD_ID` | bot | *(unset)* | Guild whose roles mirror ranks. Unset ⇒ the bot never touches roles. |
| `ROLE_SYNC_SECONDS` | bot | `900` | How often to reconcile rank roles. Floor of 60. |

### `TRUST_FORWARDED_FOR` is a security decision

Set it to `true` **only** when a reverse proxy you control overwrites
`X-Forwarded-For`. If the header reaches the app unmodified from the
internet, any caller can set it to a random value per request and walk
straight past the rate limiter.

With Caddy, the `reverse_proxy` directive sets it correctly, so `true`
is right behind the config in this document. Behind a naked port
exposure, leave it `false`.

### Rate limiting is per replica

The limiter's state lives in the process. Two replicas behind a load
balancer means two independent budgets. For a club-sized deployment that
is fine; if you scale out, put the authoritative limit in the reverse
proxy and treat the in-process one as a backstop.

---

## Discord rank roles

The bot mirrors each member's global rank onto a real Discord role,
which is what makes a rank socially visible rather than a string in a
database.

Setup, in the guild:

1. Create one role per rank, named **exactly** after the rank title —
   `⏳ L'Aspirant`, `👁️ Observateur des Ombres`, `🌱 L'Initié`,
   `📖 Apprenti Forgeron`, `⚡ Compagnon de Guilde`,
   `🔥 Vétéran des Arènes`, `💎 Maître Artisan`, `🌙 Ancien de la Forge`,
   `🌟 Légende Vivante`, `👑⚡ Mythe de la Guilde`.
   Give them whatever colours and permissions you like.
2. Place the bot's own role **above** all of them in the hierarchy —
   Discord will not let it assign a role that outranks its own.
3. Set `DISCORD_GUILD_ID` and restart the bot.

The bot looks roles up **by name** and never creates them. A rank with
no matching role is simply skipped, so you can adopt this gradually. Any
member whose role assignment fails (usually a hierarchy problem) is
logged and skipped without stopping the rest of the pass.

---

## Migration note for existing deployments

Migrations `0009`–`0011` are additive and safe to run against live data.
Two columns become **deprecated but are not dropped**, so a rollback to
the previous binary keeps working:

- `qr_tokens.token` — the plaintext JWT. No longer written. Once you are
  confident in the new build, clear it: `UPDATE qr_tokens SET token = NULL;`
- `qr_tokens.is_used` — superseded by per-member claims in `attendance`.

`0010` requires the `btree_gist` extension for the season overlap
constraint; the migration creates it, which needs a role with
`CREATE EXTENSION` rights (superuser on a stock PostgreSQL, or
`rds_superuser` equivalent on managed hosts).

Existing members keep their XP and ranks. They will, however, show
`streak_days = 0` until their next XP event, because the streak was
never actually computed before this release.
