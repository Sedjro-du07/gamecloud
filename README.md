# GameCloud OS

L'OS gamifié de l'association game-dev d'Epitech Bénin.

A Rust full-stack platform — Axum 0.8 + Leptos 0.8 hybrid SSR for the
web, Serenity 0.12 + Poise 0.6 for the Discord bot — backed by
PostgreSQL and Supabase Storage.

## Layout

```
gamecloud-os/
├── Cargo.toml                       # workspace + cargo-leptos config
├── crates/
│   ├── shared/                      # pure domain layer
│   ├── web/                         # Axum + Leptos hybrid (lib + bin)
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
createdb gamecloud_dev
cp .env.example .env  # then edit DATABASE_URL etc.

cargo leptos watch                    # web (port 3000)
cargo run -p gamecloud-bot            # bot (in another terminal)
```

## Verification

The workspace is verified by:

```bash
cargo test -p gamecloud-shared --features server
cargo test -p gamecloud-web    --features ssr --no-default-features
cargo test -p gamecloud-bot

cargo clippy -p gamecloud-shared --features server                -- -D warnings
cargo clippy -p gamecloud-web    --features ssr --no-default-features -- -D warnings
cargo clippy -p gamecloud-bot                                     -- -D warnings
```

All three crates compile clean with `clippy::pedantic` enforced.

## Documentation

- **`docs/ARCHITECTURE.md`** — global topology, request flows,
  security model.
- **`docs/DATABASE_SCHEMA.md`** — every table, column, and constraint.
- **`docs/API_REFERENCE.md`** — every HTTP endpoint, body, and error
  code.
- **`docs/ROLES_PERMISSIONS.md`** — the permission matrix.
- **`docs/DEPLOYMENT.md`** — env vars, systemd units, Caddy config.
- **`docs/CONTRIBUTING.md`** — local setup, conventions, PR checklist.

## License

MIT. See `LICENSE`.
