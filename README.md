# GameCloud OS

L'OS gamifié de l'association game-dev d'Epitech Bénin.

A Rust full-stack platform — Axum 0.8 + Leptos 0.8 hybrid SSR for the
web, Serenity 0.12 + Poise 0.6 for the Discord bot — backed by
PostgreSQL and Supabase Storage.

## What it does

GameCloud OS turns association membership into an RPG, and harvests the
XP from work members are already doing rather than asking them to report
it.

- **Identity.** Discord OAuth, then a verified `@epitech.eu` address via
  a 6-digit OTP, then a track. Only verified members earn XP.
- **XP, automatically.** GitHub webhooks (commits, merged PRs, reviews,
  issues), QR attendance at sessions and game jams, and a DraftBot
  level-up forwarder from Discord.
- **Three orthogonal role axes.** A global rank earned from XP, a
  sub-role in each of the 8 production tracks, and an appointed Bureau
  role. Permissions are computed across all three.
- **Seasons.** The live leaderboard is scoped to the open season, so a
  member joining in September has a board they can actually win. Ranks,
  levels and badges stay cumulative forever.
- **Quests.** The Bureau opens a weekly objective; progress advances
  itself from the XP events already flowing in.
- **Projects.** A build is judged once per concerned track, and a
  rejection must carry written feedback. Releasing pays every
  contributor and writes the Hall of Fame entry.
- **Badges, streaks, resources, audit trail.**

## Layout

```
gamecloud-os/
├── Cargo.toml                       # workspace + cargo-leptos config
├── crates/
│   ├── shared/                      # pure domain layer
│   │   ├── account.rs               # type-state account stages
│   │   ├── badges.rs                # badge award rules
│   │   ├── projects.rs              # project lifecycle state machine
│   │   ├── roles.rs                 # tracks, bureau roles, ranks, permissions
│   │   └── xp.rs                    # XP economy, caps, streaks, levels
│   ├── web/                         # Axum + Leptos hybrid (lib + bin)
│   │   ├── api/                     # view models shared by both targets
│   │   ├── db/queries/              # one module per domain
│   │   ├── middleware/              # auth, rate limit, HMAC
│   │   ├── routes/                  # Axum handlers
│   │   ├── server_fns.rs            # Leptos server functions
│   │   ├── components/ + pages/     # the UI
│   │   └── services/                # JWT, OTP, mail, notifications
│   └── bot/                         # Discord bot binary
├── migrations/                      # sqlx-managed PostgreSQL schema
└── docs/
    ├── ARCHITECTURE.md
    ├── DATABASE_SCHEMA.md
    ├── API_REFERENCE.md
    ├── ROLES_PERMISSIONS.md
    ├── DEPLOYMENT.md
    └── CONTRIBUTING.md
```

## Quickstart

```bash
cargo install --locked cargo-leptos sqlx-cli
rustup target add wasm32-unknown-unknown
createdb gamecloud_dev
cp .env.example .env  # then edit DATABASE_URL and generate the secrets

cargo leptos watch                    # web (port 3000)
cargo run -p gamecloud-bot            # bot (in another terminal)
```

**The server will refuse to start** until `JWT_SECRET`,
`DRAFTBOT_API_KEY` and `GITHUB_WEBHOOK_SECRET` are real generated values
rather than the placeholders in `.env.example`. Generate each with
`openssl rand -base64 48`. See `QUICKSTART.md` for the full walkthrough.

## Verification

The workspace is verified by:

```bash
cargo test -p gamecloud-shared --features server
cargo test -p gamecloud-web    --features ssr --no-default-features
cargo test -p gamecloud-bot

cargo clippy -p gamecloud-shared --features server --all-targets                -- -D warnings
cargo clippy -p gamecloud-web    --features ssr --no-default-features --all-targets -- -D warnings
cargo clippy -p gamecloud-bot    --all-targets                                  -- -D warnings

# The WASM half of the hybrid crate must compile too:
cargo check -p gamecloud-web --features hydrate --no-default-features \
    --target wasm32-unknown-unknown
```

149 tests pass and all three crates compile clean with
`clippy::pedantic` enforced.

## Documentation

- **`docs/ARCHITECTURE.md`** — global topology, request flows,
  security model.
- **`docs/DATABASE_SCHEMA.md`** — every table, column, and constraint.
- **`docs/API_REFERENCE.md`** — every HTTP endpoint, body, and error
  code.
- **`docs/ROLES_PERMISSIONS.md`** — the permission matrix and where each
  check is enforced.
- **`docs/DEPLOYMENT.md`** — env vars, systemd units, Caddy config.
- **`docs/CONTRIBUTING.md`** — local setup, conventions, PR checklist.
- **`CHANGELOG.md`** — what changed and why.

## License

MIT. See `LICENSE`.
