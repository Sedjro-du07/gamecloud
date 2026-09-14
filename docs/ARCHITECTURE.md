# GameCloud OS — Architecture

This document is the high-level map of the GameCloud OS codebase. It
covers component boundaries, runtime topology, the data flow for every
critical user journey, and the security model that ties them together.

> **Status, September 2026.** Feature-complete against this document.
> Every route listed below is registered, every table in
> `DATABASE_SCHEMA.md` is read or written by code, and the middleware
> stack is the one described here. Earlier revisions of this file
> advertised endpoints and a rate-limit layer that did not exist; that
> drift is gone.

---

## 1. Components

```
                                                      ╔══════════════════╗
                                                      ║   Discord API    ║
                                                      ║  (gateway+REST)  ║
                                                      ╚════════╤═════════╝
                                                               │
                                                  shards ╲     │     ╱ HTTP
                                                          ╲    │    ╱
┌──────────────────────────┐                            ╔═════════════════╗
│        Browser           │                            ║  gamecloud-bot  ║
│  (Leptos WASM hydrate)   │◄── HTML + JSON ─┐          ║   (Serenity +   ║
└────────────┬─────────────┘                 │          ║      Poise)     ║
             │                               │          ╚════════╤════════╝
             │  HTTPS                        │                   │
             ▼                               │                   │
   ┌─────────────────────┐                   │                   │
   │  Cloudflare / nginx │                   │                   │
   │   (TLS, rate limit, │                   │                   │
   │    static assets)   │                   │                   │
   └──────────┬──────────┘                   │                   │
              │                              │                   │
              ▼                              │                   │
   ╔══════════════════════════════════╗      │                   │
   ║         gamecloud-web            ║──────┘                   │
   ║  (Axum 0.8, single binary,       ║                          │
   ║  serves SSR HTML + REST + WASM)  ║◄─────────────────────────┘
   ║                                  ║         shared SQL pool
   ║   ┌────────────────────────┐     ║
   ║   │ Routes                 │     ║
   ║   │  - /api/auth/*         │     ║
   ║   │  - /api/users/*        │     ║
   ║   │  - /api/projects/*     │     ║
   ║   │  - /api/quests/*       │     ║
   ║   │  - /api/resources/*    │     ║
   ║   │  - /api/seasons/*      │     ║
   ║   │  - /api/qr/*           │     ║
   ║   │  - /api/admin/*        │     ║
   ║   │  - /api/webhooks/*     │     ║
   ║   │  - /api/sync/*         │     ║
   ║   │  - /  (Leptos SSR)     │     ║
   ║   └─────────┬──────────────┘     ║
   ║             │                    ║
   ║   ┌─────────▼──────────────┐     ║
   ║   │ Tower middleware stack │     ║
   ║   │  - tracing             │     ║
   ║   │  - cors (PUBLIC_ORIGIN)│     ║
   ║   │  - compression         │     ║
   ║   │  - body size limit     │     ║
   ║   │  - rate limit (per-IP, │     ║
   ║   │    2 buckets)          │     ║
   ║   │  - HMAC signature      │     ║
   ║   │    (in the handler)    │     ║
   ║   │  - JWT auth + Authority│     ║
   ║   │    guard (extractors)  │     ║
   ║   └─────────┬──────────────┘     ║
   ║             │                    ║
   ║   ┌─────────▼──────────────┐     ║
   ║   │ gamecloud-shared       │     ║
   ║   │ (domain types, roles,  │     ║
   ║   │  permissions, XP)      │     ║
   ║   └─────────┬──────────────┘     ║
   ╚═════════════│════════════════════╝
                 │
        ┌────────┼────────────────────┐
        │        │                    │
        ▼        ▼                    ▼
 ┌────────────┐  ┌────────────┐  ┌─────────────────┐
 │ PostgreSQL │  │  Supabase  │  │   GitHub API    │
 │  (sqlx)    │  │  Storage   │  │  (octocrab) +   │
 │            │  │            │  │   webhooks      │
 └────────────┘  └────────────┘  └─────────────────┘
```

## 2. Workspace layout

```
gamecloud-os/
├── Cargo.toml                       # workspace + cargo-leptos metadata
├── crates/
│   ├── shared/                      # pure domain layer (no IO)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── account.rs           # type-state stages
│   │       ├── badges.rs            # badge award rules
│   │       ├── errors.rs
│   │       ├── models.rs            # DB row mirrors
│   │       ├── projects.rs          # project lifecycle state machine
│   │       ├── roles.rs             # roles, ranks, permissions
│   │       └── xp.rs                # XP economy, caps, streaks, levels
│   ├── web/                         # cargo-leptos hybrid lib + bin
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs               # WASM entry point (hydrate)
│   │       ├── bin/server.rs        # native server binary
│   │       ├── config.rs            # env vars (rejects placeholder secrets)
│   │       ├── api/                 # view models shared by both targets
│   │       ├── middleware/          # auth, rate_limit, github_signature
│   │       ├── routes/              # Axum handlers
│   │       ├── server_fns.rs        # Leptos server functions
│   │       ├── components/ pages/   # Leptos views
│   │       ├── services/            # jwt, otp, mailer, notifications
│   │       └── db/queries/          # sqlx queries, one module per domain
│   └── bot/                         # Discord bot binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── commands/
│           ├── events/
│           └── notifier.rs
├── migrations/                      # sqlx migrations (numbered)
│   ├── 0001_init.sql
│   ├── 0002_users.sql
│   ├── 0003_tracks_and_badges.sql
│   ├── 0004_projects.sql
│   ├── 0005_resources_attendance.sql
│   └── 0006_progression.sql
└── docs/
    ├── ARCHITECTURE.md              # ← you are here
    └── DATABASE_SCHEMA.md
```

### Crate boundaries

- **`gamecloud-shared`** is a pure-Rust domain layer. It contains
  zero IO and zero framework-specific code. It exposes:
  - `models` — exact mirrors of every database row, with optional
    `sqlx::FromRow` derives behind the `server` feature.
  - `roles` — the three orthogonal role dimensions, the permission
    matrix, and `Authority::can` for runtime checks.
  - `xp` — every numerical constant from the XP economy.
  - `account` — the type-state encoding of account stages
    (`Pending` → `EmailSubmitted` → `Verified`).

  Both `crates/web` and `crates/bot` depend on it.

- **`gamecloud-web`** is a single hybrid crate compiled twice by
  `cargo-leptos`: once to WASM (lib target, `hydrate` feature) and once
  to a native binary (`server` target, `ssr` feature). Server-only code
  is gated behind `#[cfg(feature = "ssr")]` so the WASM bundle stays
  small.

- **`gamecloud-bot`** is a separate native binary so it can be deployed
  and restarted independently of the web server. It connects to the
  same PostgreSQL and writes to the same XP and audit tables — the bot
  is *not* a thin client of the web API; it owns its own DB pool. This
  is deliberate: a Discord disruption should not cascade to the web,
  and an Axum redeploy should not silently drop Discord events.

## 3. Authentication flow

```
Browser                  Axum                    Discord                 Postgres
   │                       │                        │                        │
   │ GET /login            │                        │                        │
   ├──────────────────────►│                        │                        │
   │                       │ build OAuth URL +      │                        │
   │                       │ random state cookie    │                        │
   │ 302 to Discord ◄──────┤                        │                        │
   │                       │                        │                        │
   │ Discord consent UI    │                        │                        │
   ├───────────────────────┼───────────────────────►│                        │
   │                       │                        │                        │
   │ GET /callback?code=…  │                        │                        │
   ├──────────────────────►│                        │                        │
   │                       │ POST oauth/token       │                        │
   │                       ├───────────────────────►│                        │
   │                       │ access_token, user info│                        │
   │                       │◄───────────────────────┤                        │
   │                       │ UPSERT user (Pending)  │                        │
   │                       ├────────────────────────┼───────────────────────►│
   │                       │                        │                        │
   │                       │ issue JWT access (1h)  │                        │
   │                       │ + refresh (7d) cookies │                        │
   │ 302 → /onboarding/email                        │                        │
   │◄──────────────────────┤                        │                        │
   │                       │                        │                        │
   │ POST /api/auth/email  │                        │                        │
   ├──────────────────────►│                        │                        │
   │                       │ regex check            │                        │
   │                       │ argon2(otp)            │                        │
   │                       │ INSERT email_otps      │                        │
   │                       ├────────────────────────┼───────────────────────►│
   │                       │ send email (SMTP)      │                        │
   │ 200                   │                        │                        │
   │◄──────────────────────┤                        │                        │
   │                       │                        │                        │
   │ POST /api/auth/verify │                        │                        │
   ├──────────────────────►│                        │                        │
   │                       │ verify hash, attempts++│                        │
   │                       │ UPDATE users           │                        │
   │                       │   email_verified=true  │                        │
   │                       │   global_rank=Visitor  │                        │
   │                       ├────────────────────────┼───────────────────────►│
   │                       │ DELETE email_otps      │                        │
   │                       │ rotate JWT             │                        │
   │ 200 + Set-Cookie      │                        │                        │
   │◄──────────────────────┤                        │                        │
```

Token strategy:

| Token             | Storage                                 | Lifetime |
| ----------------- | --------------------------------------- | -------- |
| Access JWT        | HttpOnly Secure SameSite=Lax cookie     | 1 hour   |
| Refresh token     | HttpOnly Secure SameSite=Strict cookie  | 7 days   |
| OAuth state       | HttpOnly cookie, deleted on callback    | 10 min   |
| Email OTP         | Argon2 hash in `email_otps`             | 15 min   |

The refresh token is stored *hashed* in the `refresh_tokens` table
(SHA-256). Plaintext tokens are issued exactly once at login or rotation
and never persisted. Rotation is mandatory on every refresh; the old
row is marked `revoked = true` rather than deleted, so an attempted
reuse can be detected and trigger a forced logout of the user across
all sessions.

## 4. XP grant flow (GitHub webhook)

```
GitHub                  Axum                              Postgres
   │                      │                                  │
   │ POST /webhooks/github│                                  │
   │  + X-Hub-Signature-256                                  │
   ├─────────────────────►│                                  │
   │                      │ HMAC-SHA256 verify (Tower layer) │
   │                      │ ── reject if bad sig ──          │
   │                      │                                  │
   │                      │ match event:                     │
   │                      │   push / PR / issue / review     │
   │                      │                                  │
   │                      │ resolve user by github_username  │
   │                      ├─────────────────────────────────►│
   │                      │ ── if not found: drop event ──   │
   │                      │                                  │
   │                      │ apply daily-cap rules            │
   │                      │ (count xp_logs today by source)  │
   │                      ├─────────────────────────────────►│
   │                      │                                  │
   │                      │ apply streak + multi-track       │
   │                      │ multipliers (gamecloud-shared)   │
   │                      │                                  │
   │                      │ BEGIN TX                         │
   │                      ├─────────────────────────────────►│
   │                      │   INSERT xp_logs                 │
   │                      │   UPDATE users.xp_total          │
   │                      │   UPDATE users.global_rank       │
   │                      │   UPDATE track_memberships       │
   │                      │   INSERT audit_logs              │
   │                      │ COMMIT                           │
   │                      │                                  │
   │                      │ enqueue Discord notification     │
   │                      │ (cross-process queue / DB row)   │
   │                      │                                  │
   │ 200 OK ◄─────────────┤                                  │
```

Cross-process Discord notifications use a database queue table
(introduced as needed in phase 3) rather than an in-memory channel.
This way the bot can pick up notifications even if it was offline at the
time the webhook fired.

## 5. QR-based attendance flow

```
Admin browser            Axum                   Postgres
    │                      │                       │
    │ POST /api/qr/generate│                       │
    │  body: event details │                       │
    ├─────────────────────►│                       │
    │                      │ guard: event_manager  │
    │                      │ build JWT             │
    │                      │   {jti, event, exp}   │
    │                      │ INSERT qr_tokens      │
    │                      ├──────────────────────►│
    │                      │                       │
    │ {token, png}         │                       │
    │◄─────────────────────┤                       │
    │                      │                       │
    │ /qr/display (full screen, projector)         │

Member browser           Axum                   Postgres
    │                      │                       │
    │ POST /api/qr/scan    │                       │
    │  body: token         │                       │
    ├─────────────────────►│                       │
    │                      │ jwt verify            │
    │                      │ SELECT qr_tokens      │
    │                      │   FOR UPDATE          │
    │                      ├──────────────────────►│
    │                      │ ── reject if used ── │
    │                      │ INSERT attendance     │
    │                      │ UPDATE qr_tokens.used │
    │                      │ INSERT xp_logs        │
    │                      │ UPDATE users.xp_total │
    │ 200 + xp gain        │                       │
    │◄─────────────────────┤                       │
```

The `FOR UPDATE` lock plus the `is_used` flag together prevent the
classic double-scan race.

## 6. Project validation flow

```
Author          Reviewer (track A)      Reviewer (track B)        TrackLead
  │                   │                       │                       │
  │ Draft → InReview  │                       │                       │
  │── creates rows in track_validations (one per concerned track)     │
  │                   │                       │                       │
  │                   │ Approve A             │                       │
  │                   │ ── status: Approved   │                       │
  │                   │ ── audit_logs row     │                       │
  │                   │ ── XP for reviewer    │                       │
  │                   │                       │                       │
  │                   │                       │ Approve B             │
  │                   │                       │                       │
  │ ── all approvals received → transition status to Approved         │
  │                                                                   │
  │                                                                   │ Publish
  │                                                                   │ Released
  │── XP distributed to all contributors                              │
  │── hall_of_fame entry created                                      │
  │── Discord embed posted                                            │
```

Per-track validations have their own row, so concurrent reviewers do
not contend for the same row. The transition `PartialOK` → `Approved`
is computed from the count of non-`Pending` validations.

## 7. Discord ↔ Platform sync

The bot subscribes to:

- `MessageCreate` — to intercept DraftBot level-up embeds. The bot
  parses the embed (regex on the `description` field), maps the Discord
  user to a platform user via `users.discord_id`, and writes an
  `xp_logs` row with `source = 'Discord'`.
- `ReactionAdd` — when a designated emoji (e.g. ⭐) is added by a
  member with the `Validator` privilege to a help message, the message
  author receives `XP_DISCORD_HELP_VALIDATED`.
- `GuildMemberAdd` — to start the Discord-side onboarding (DM with the
  registration link).

The bot exposes Poise slash commands (`/profil`, `/xp`, `/track`,
`/leaderboard`, …) which read directly from PostgreSQL — they do not go
through the Axum API. This avoids a second hop and the need for an
internal service token.

When the *web* needs to push an embed (e.g. a project goes Released),
it writes a row into a `notifications_outbox` table (introduced in
phase 3); the bot polls this table every few seconds and consumes it
transactionally. This decouples the two binaries' uptime.

## 8. Storage

| Asset                | Location                                           |
| -------------------- | -------------------------------------------------- |
| Project files        | `supabase://projects/{project_id}/v{n}/{filename}` |
| Project screenshots  | `supabase://projects/{project_id}/screenshots/`    |
| User custom avatars  | `supabase://avatars/{user_id}/`                    |
| Project thumbnails   | `supabase://projects/{project_id}/thumb`           |

Supabase Storage URLs are presigned for upload (valid 5 minutes).
Uploads happen browser → Supabase directly; the Axum server only
brokers the presigned URL and records the metadata once the client
notifies it. This keeps the Axum process out of the bandwidth path.

## 9. Security model

| Surface                    | Defense                                                 |
| -------------------------- | ------------------------------------------------------- |
| `/api/webhooks/github`     | HMAC-SHA256 verification (Tower layer), constant-time   |
| `/api/sync/draftbot`       | `X-API-Key` header, server-side rotation                |
| Admin & Bureau routes      | JWT + role guard; permission via `Authority::can`       |
| Track-Lead routes          | JWT + per-track `TrackRole >= Lead` guard               |
| Email field                | Regex + DB CHECK constraint + Argon2 OTP                |
| File uploads               | MIME sniff + ≤500 MB cap (handler) + DB CHECK           |
| Sessions                   | 1h JWT + 7d hashed refresh token, rotation on use       |
| Rate limiting              | `middleware::rate_limit`, fixed window per IP, two buckets |
| Audit logs                 | Append-only, every privileged action recorded           |
| Type-state                 | `User<Verified>` cannot be constructed without OTP pass |

The `Authority::can` matrix in `gamecloud-shared` is the *single*
authoritative source of truth for permissions. Both web handlers and
bot commands consult it; no permission decision should ever be inlined
elsewhere.

## 10. Observability

- All HTTP requests are traced with `tower-http`'s `TraceLayer`.
- The bot uses `tracing-subscriber` with the same JSON format.
- Every log line carries `request_id` (web) or `event_id` (bot) so a
  single user action can be correlated across the two binaries.
- Database operations log slow queries (>200 ms) at WARN.
- Audit logs are *not* observability — they are user-visible records,
  written transactionally with the action they describe.

## 11. Deployment topology (target state)

```
                           ┌──────────────────────┐
                           │   Cloudflare DNS     │
                           │  + WAF + rate limit  │
                           └──────────┬───────────┘
                                      │
                                      ▼
                            ┌─────────────────┐
                            │   nginx / Caddy │
                            │   TLS, gzip     │
                            └────────┬────────┘
                                     │
                  ┌──────────────────┼──────────────────┐
                  │                                     │
                  ▼                                     ▼
         ┌────────────────┐                    ┌────────────────┐
         │ gamecloud-web  │     systemd        │ gamecloud-bot  │
         │ (3000)         │   restart=always   │ (no port)      │
         └────────┬───────┘                    └────────┬───────┘
                  │                                     │
                  └──────────────┬──────────────────────┘
                                 │
                                 ▼
                       ┌─────────────────┐
                       │   PostgreSQL    │
                       │  (managed,      │
                       │   point-in-time │
                       │   backups)      │
                       └─────────────────┘
```

Phase 5 documents the exact deploy steps (`DEPLOYMENT.md`).

---

## 12. The XP engine

Every XP award on the platform funnels through
`db::queries::xp::grant`. This is the single most important invariant in
the codebase, and it was not always true: the webhook handler and the QR
scan used to have separate implementations, and the QR one never
recomputed the member's rank, so attendance XP silently promoted nobody.

One award triggers this cascade, in one transaction:

```
   XpGrant { user_id, base, source, track?, cap?, apply_multipliers }
        │
        ▼
   1. load member  ──► streak_days, active_tracks, rank, level, xp_total
        │
        ▼
   2. multipliers   base × streak(1.0–2.0) × multi_track(1.0–1.30)
        │
        ▼
   3. daily cap     trim so today's total for this bucket ≤ limit
        │           ── ORDER MATTERS: capping before multiplying turns a
        │              50/day cap into 130/day
        ▼
   4. ledger        INSERT xp_logs (… , season_id)
        │
        ▼
   5. member        xp_total, global_rank, level, streak, longest_streak
        │           ── rank via GlobalRank::from_xp, gated on
        │              email_verified and on onboarding having happened
        ▼
   6. track pool    track_xp + auto track_role (Lead/CoLead untouched)
        │
        ▼
   7. quests        advance matching counters; pay out any that complete
        │           (quest XP is exact: no multipliers, no cap, no
        │            recursion back into this engine)
        ▼
   8. badges        rebuild the snapshot, INSERT … ON CONFLICT DO NOTHING
        │
        ▼
   9. announce      notifications_outbox rows for rank-up / badge / quest
```

Because the whole cascade is one transaction, a member can never see a
rank-up announced for XP that rolled back, nor keep XP whose badge write
failed.

`grant` opens its own transaction; `grant_in_tx` joins one the caller
owns, which is how a QR scan stays atomic with its `attendance` row and
a project release pays every contributor at once.

---

## 13. Seasons

A season is a time window that scopes the leaderboard. The schema
enforces that at most one is open at any instant (an exclusion
constraint over `tstzrange(starts_at, ends_at)`), so "the current
season" is never ambiguous.

Every `xp_logs` row is stamped with the season that was open when it was
written. That makes the season board a single `WHERE season_id = $1`
aggregate, and leaves ranks, levels and badges cumulative across seasons
— a new season changes what the board *shows*, never what a member has
earned.

The motivation is retention, not novelty: an all-time board freezes.
The members who founded the club sit on top of it permanently, and a
first-year joining in September can see at a glance that they will never
catch up.

---

## 14. Rate limiting

`middleware::rate_limit` applies a fixed-window counter per
`(bucket, client IP)`:

| Bucket | Paths | Default |
|---|---|---|
| `Sensitive` | `/api/auth/{email,verify,refresh,login}`, `/api/qr/scan`, `/api/sync/draftbot` | 10/min |
| `Default` | everything else | 120/min |

State is in-process, so a multi-replica deployment limits per replica;
the reverse proxy is the right place for a global limit, and
`DEPLOYMENT.md` says so. The client is the peer address, or the first
hop of `X-Forwarded-For` when `TRUST_FORWARDED_FOR=true` — which should
only ever be set when a proxy you control overwrites that header.

A fixed window lets a caller burst up to 2× the limit across a boundary.
That is an acceptable trade for a club-sized deployment, and the numbers
are chosen with the slack in mind; the alternative, a sliding log, costs
memory proportional to request volume for accuracy nobody here needs.
