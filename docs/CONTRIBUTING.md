# Contributing to GameCloud OS

Welcome. This guide describes how to set up the workspace locally,
the conventions the codebase follows, and what every PR is expected
to do before review.

## Local setup

### Prerequisites

- Rust stable (≥ 1.80) — install via rustup
- `cargo-leptos` — `cargo install --locked cargo-leptos`
- `sqlx-cli` — `cargo install --locked sqlx-cli --no-default-features --features postgres,rustls`
- PostgreSQL 15+ (locally or via Docker)

### First run

```bash
git clone https://github.com/gamecloud-os/gamecloud-os
cd gamecloud-os

# Database
createdb gamecloud_dev
cp .env.example .env   # then edit DATABASE_URL etc.

# Compile + watch on the web binary
cargo leptos watch

# In another terminal: compile + run the bot
cargo run -p gamecloud-bot
```

By default the web binary applies migrations on startup, so a fresh
database becomes ready on first boot.

## Workspace layout

See `docs/ARCHITECTURE.md` § 2 for the full tree. Briefly:

```
crates/
  shared/   # pure domain types — no IO, no framework
  web/      # Axum + Leptos hybrid (one crate, two compile targets)
  bot/      # Serenity + Poise Discord bot
migrations/ # sqlx-managed numbered .sql files
docs/       # what you're reading
```

## Code conventions

### Rust

- `#![warn(clippy::pedantic)]` is on at the crate level. We apply
  targeted `#[allow]` attributes when a pedantic lint hurts
  readability — never blanket allows in functions.
- Public items have doc comments. `#![warn(missing_docs)]` is on.
- No `unwrap()` outside of tests, examples, or one-shot bootstrap
  paths. Return `Result` and let the error envelope decide the
  status code.
- Errors are `thiserror::Error` enums. Domain errors live in
  `gamecloud_shared::DomainError`, infrastructure errors live in
  per-crate types and wrap `DomainError`.
- Async is `tokio` everywhere. No `async-std`, no `smol`.
- 2 tabs of indentation? Never. We use 4 spaces, `rustfmt` defaults.

### SQL

- Migrations are append-only; we never edit a committed migration
  file. To change a schema, write a new migration that ALTERs.
- Every numerical/state column has a `CHECK` constraint that pins its
  allowed values. The database is the last line of defense.
- Indexes live in the migration that creates the table they cover,
  unless they are added later for a specific perf reason.

### Frontend

- Leptos 0.8 + `leptos_router` + `leptos_meta`.
- Components live in `crates/web/src/components/`, pages in
  `crates/web/src/pages/`. The router declares them in
  `crates/web/src/app.rs`.
- CSS is plain SCSS in `crates/web/style/main.scss`. We use design
  tokens in `:root` and BEM-ish class names (`.gc-card`,
  `.gc-card__avatar`). No Tailwind.

## Branches & commits

- `main` is always green: it builds, tests pass, clippy passes.
- Feature branches are named `feat/<short-slug>`,
  `fix/<short-slug>`, or `docs/<short-slug>`.
- Commits use the imperative mood: "Add QR scan endpoint", not
  "Added" or "Adding".
- One logical change per commit. We do squash + rebase, not merge
  commits.

## What to do before opening a PR

The CI runs the same three checks; failing locally first saves a round
trip.

```bash
# 1. Compile every crate in every relevant configuration.
cargo check -p gamecloud-shared --features server
cargo check -p gamecloud-shared
cargo check -p gamecloud-web --features ssr --no-default-features
cargo check -p gamecloud-bot

# 2. Run all tests.
cargo test -p gamecloud-shared --features server
cargo test -p gamecloud-web --features ssr --no-default-features
cargo test -p gamecloud-bot

# 3. Clippy strict.
cargo clippy -p gamecloud-shared --features server -- -D warnings
cargo clippy -p gamecloud-web --features ssr --no-default-features -- -D warnings
cargo clippy -p gamecloud-bot -- -D warnings

# 4. Format.
cargo fmt --all
```

If you touched migrations, also run:

```bash
sqlx migrate run --database-url $DATABASE_URL
```

against a freshly-created database to make sure the new migration
applies cleanly to a virgin schema.

## Adding a new endpoint

The recipe used by every existing endpoint:

1. Add or extend a query function in
   `crates/web/src/db/queries/<topic>.rs`. This function takes a
   `&PgPool` (or `&mut Transaction`) and returns `WebResult<T>`.
2. Add the handler in `crates/web/src/routes/<topic>.rs`. Use
   `CurrentUser` + `users::load_authority` + `Authority::can(Action)`
   to gate access.
3. Wire the handler into the router in the module's `router()`
   function. The top-level merger lives in `crates/web/src/router.rs`.
4. Document the endpoint in `docs/API_REFERENCE.md`.
5. Write at least one unit test for the query function and one for
   any non-trivial logic in the handler.

## Adding a new domain enum or constant

Domain types and XP constants live in `gamecloud_shared`. The
checklist:

1. Add the type to the appropriate module (`roles.rs`, `xp.rs`, etc.).
2. If it gets stored in the DB, also extend the `CHECK` constraint
   in the relevant migration **with a new migration file**.
3. If it shows up in the API, document the allowed values in
   `docs/DATABASE_SCHEMA.md` § "Allowed enum values reference".
4. Add unit tests on any new permission decisions.

## Code review

- A reviewer is expected to run the test suite locally. Don't trust CI
  alone for security-sensitive changes (auth, permission matrix,
  webhook verification).
- We squash on merge. Write your final commit message on the PR
  description.

## Where to ask questions

For now, in the GameCloud Discord `#dev` channel. If a question
recurs, fold the answer into this document.

---

## Conventions added in September 2026

### Never bypass the XP engine

Every XP award goes through `db::queries::xp::grant` (or `grant_in_tx`
when it must be atomic with something else). Do not write `xp_logs` or
`users.xp_total` directly.

This is not style. There used to be two paths — the webhook handler and
the QR scan — and the second one skipped the rank recomputation, so
attendance XP silently promoted nobody for months. The engine owns
multipliers, caps, the rank ladder, levels, streaks, track pools, quest
progress, badges and announcements. Bypassing it means silently skipping
some of that.

### The shared crate owns the vocabulary

`GlobalRank`, `BureauRole`, `Track`, `TrackRole`, `SpecialBadge`,
`ProjectStatus` and `Verdict` each have `as_str` and `parse` in
`gamecloud-shared`. Use them.

Before this release there were four separate `match s { "Visitor" => …
}` blocks scattered across the web and bot crates, which is exactly how
a rename turns into a silent bug. If you need a new string ↔ enum
mapping, put it on the enum.

### No "anchor imports"

Do not write `let _ = (some_fn, other_fn);` or
`const _ANCHOR: StatusCode = …` to keep an unused import alive. Delete
the import. The codebase had four of these and they each hid a real
piece of dead code.

### Both targets must compile

`crates/web` builds twice. A change that compiles under `ssr` can easily
break `hydrate` — anything touching `api/`, `components/`, `pages/` or a
server function signature. Check both:

```bash
cargo check -p gamecloud-web --features ssr --no-default-features
cargo check -p gamecloud-web --features hydrate --no-default-features \
    --target wasm32-unknown-unknown
```

View models in `api/` must stay free of `chrono` types and borrowed
data: timestamps cross the wire as pre-formatted strings.

### Test the logic, not the framework

Pure domain logic belongs in `gamecloud-shared` where it can be tested
without a database — that is why the project state machine, the badge
rules, the streak arithmetic and the level curve all live there. A
handler should be thin enough that its own tests are about validation
and error mapping.

### Clippy is pedantic and denied

```bash
cargo clippy -p <crate> --all-targets -- -D warnings
```

`--all-targets` matters: it lints the test modules too. When an
`#[allow]` is genuinely right, write a comment saying *why*, as the
existing ones do. An `#[allow]` without a reason will be questioned in
review.
