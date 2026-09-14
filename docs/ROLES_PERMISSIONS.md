# GameCloud OS — Roles & Permissions

This document is the human-readable counterpart of the
`Authority::can(Action)` matrix in `gamecloud_shared::roles`. If the
two ever disagree, the code is authoritative — but PRs that change one
must change the other in the same commit.

## The three role dimensions

A user's permissions are determined by three orthogonal dimensions:

1. **Global rank** — derived from total XP, gated by email verification.
2. **Bureau role** — at most one elected/appointed leadership position.
3. **Track memberships** — for each track joined, a sub-role from
   `Observer` to `Lead`.

`Authority::can(Action)` consults all three to make a single decision.

## Global ranks

| Rank | XP threshold | Gate | Title |
|------|-------------:|------|-------|
| `Pending` | 0 | No email submitted | ⏳ L'Aspirant |
| `Visitor` | 0 | Email submitted, not verified | 👁️ Observateur des Ombres |
| `Initiate` | 0 | Email verified, onboarding done | 🌱 L'Initié |
| `Apprentice` | 150 | | 📖 Apprenti Forgeron |
| `JuniorDev` | 400 | | ⚡ Compagnon de Guilde |
| `SeniorDev` | 1 000 | | 🔥 Vétéran des Arènes |
| `Expert` | 2 500 | | 💎 Maître Artisan |
| `Veteran` | 5 000 | | 🌙 Ancien de la Forge |
| `Legend` | 10 000 | | 🌟 Légende Vivante |
| `Myth` | 25 000 | | 👑⚡ Mythe de la Guilde |

Source: `gamecloud_shared::xp::RANK_THRESHOLDS` and
`GlobalRank::from_xp`.

> A user above `Visitor` *must* have `email_verified = true`. This is
> enforced both by the type-state pattern (`User<Verified>` is the
> only stage that can earn XP) and by a `CHECK` constraint on the
> `users` table.

## Track sub-roles

Each user joins 1..=8 tracks. Inside a track they hold one role:

| Role | XP threshold (auto) | Manual? | Permissions inside the track |
|------|--------------------:|---------|------------------------------|
| `Observer` | 0 | auto | Read-only access to track resources |
| `Contributor` | 100 | auto | Submit projects |
| `Reviewer` | 300 | auto | Review submitted projects |
| `Mentor` | 500 | auto | Mentor juniors, validate junior reviews |
| `CoLead` | — | manual (by Lead) | Co-lead the track |
| `Lead` | — | manual (by Bureau) | Lead the track, publish Released |

Source: `TrackRole::from_track_xp` plus explicit appointment paths.
`CoLead` and `Lead` are *never* auto-assigned, even if the user has
1 000 000 track XP.

## Bureau roles

There are 16 Bureau roles. Six are considered **executive** and unlock
destructive operations like `GrantManualXp` and `ViewAuditLogs`.

| Role | Tier | Title |
|------|------|-------|
| `President` | exec | ⚜️ Grand Archonte |
| `VicePresident` | exec | 🛡️ Archonte Adjoint |
| `Secretary` | exec | 📋 Scribe Royal |
| `Treasurer` | exec | 💰 Intendant du Trésor |
| `VpTech` | exec | 🔧 Forgeron Suprême |
| `VpCommunity` | exec | 🌐 Héraut en Chef |
| `EventManager` | non-exec | 🎪 Maître des Arènes |
| `AssistantEventManager` | non-exec | 🎭 Écuyer des Arènes |
| `Archiviste` | non-exec | 📜 Gardien des Chroniques |
| `AssistantArchiviste` | non-exec | 📂 Apprenti Chroniqueur |
| `CommunityManager` | non-exec | 📣 Éclaireur en Chef |
| `SocialMediaManager` | non-exec | 📸 Barde Numérique |
| `Moderator` | non-exec | ⚔️ Sentinelle |
| `AssistantModerator` | non-exec | 🗡️ Garde |
| `RecruitmentOfficer` | non-exec | 🎯 Chasseur de Talents |
| `PrManager` | non-exec | 🤝 Ambassadeur Suprême |

Source: `BureauRole::title()` and `BureauRole::is_executive()`.

## Permission matrix

Rows are actions (variants of the `Action` enum). Columns describe
who satisfies the predicate.

| Action | Required |
|--------|----------|
| `ViewPublicProjects` | rank ≥ `Visitor` |
| `ViewTrackInternalProjects(t)` | track-membership in `t` (any role) |
| `SubmitEpitechEmail` | rank = `Pending` |
| `CompleteOnboarding` | rank = `Visitor` |
| `CreateProject` | rank ≥ `JuniorDev` |
| `SubmitProjectForReview` | rank ≥ `JuniorDev` |
| `ReviewProjectForTrack(t)` | role in `t` ≥ `Reviewer` |
| `PublishProjectAsReleased(t)` | role in `t` ≥ `Lead` |
| `AppointTrackCoLead(t)` | role in `t` ≥ `Lead` |
| `AppointTrackLead(t)` | executive Bureau role |
| `AssignBureauRole` | `President` ∨ `VicePresident` |
| `GenerateQrToken` | `EventManager` ∨ `AssistantEventManager` ∨ executive |
| `GrantManualXp` | executive |
| `ValidateResource` | `Archiviste` ∨ `AssistantArchiviste` ∨ any track Lead |
| `ModerateContent` | `Moderator` ∨ `AssistantModerator` ∨ executive |
| `AccessAdminPanel` | any Bureau role |
| `ViewAuditLogs` | executive |

Source: `Authority::can` in `crates/shared/src/roles.rs`. Unit tests in
the same file lock in the most-tested rows.

> A `Pending` user can do exactly one thing: `SubmitEpitechEmail`.
> Every other action is denied for a `Pending` actor regardless of any
> other role they might somehow be carrying.

## Where permissions are enforced

| Surface | How |
|---------|-----|
| Web (Axum) handlers | Extract `CurrentUser`, call `users::load_authority`, then `authority.can(action)` |
| Bot (Poise) commands | Read the same `users` and `track_memberships` rows; reuse the same `Authority::can` |
| Database | `CHECK` constraints on `global_rank`, `track_role`, `bureau_role`, `email_verified` consistency |

The application code is the fast path; the database constraints are
the slow last line of defense in case a future bug bypasses the
typed wrapper.

---

## Where permissions are enforced (September 2026)

The previous revision of this section described an intent. This one
describes the code.

`Authority::can` used to be consulted in exactly **one** place in the
whole codebase (`routes::qr::generate`), which meant the 17-action
permission table above was, in practice, decorative. Every privileged
route now goes through it:

| Route | Action |
|---|---|
| `POST /api/qr/generate`, `GET /api/qr/{id}/sheet` | `GenerateQrToken` |
| `POST /api/users/me/tracks` (first track) | `CompleteOnboarding` |
| `POST /api/projects` | `CreateProject` |
| `POST /api/projects/{id}/contributors` | `CreateProject` |
| `POST /api/projects/{id}/submit` | `SubmitProjectForReview` |
| `POST /api/projects/{id}/review` | `ReviewProjectForTrack(track)` |
| `POST /api/projects/{id}/release` | `PublishProjectAsReleased(primary_track)` |
| `GET /api/projects?internal=true` | `ViewTrackInternalProjects(track)` or `AccessAdminPanel` |
| `POST /api/resources/{id}/validate`, `GET /api/resources?pending=true` | `ValidateResource` |
| `POST /api/quests` | `GrantManualXp` |
| `POST /api/admin/xp` | `GrantManualXp` |
| `POST /api/admin/bureau-role`, `/badge`, `/seasons` | `AssignBureauRole` |
| `POST /api/admin/track-role` (`Lead`) | `AppointTrackLead(track)` |
| `POST /api/admin/track-role` (other) | `AppointTrackCoLead(track)` |
| `GET /api/admin/audit` | `ViewAuditLogs` |
| `GET /api/admin/seasons` | `AccessAdminPanel` |

### Two rules worth stating explicitly

**Reviewing is per-track.** `ReviewProjectForTrack(track)` requires
`Reviewer` or above *in the track being judged*. An Audio mentor has no
standing to approve the Engineering side of a build — which is the whole
point of a multi-track review.

**Email submission is gated too.** `Action::SubmitEpitechEmail` requires
`rank == Pending`. `POST /api/auth/email` used to check only that you
were signed in, so a fully verified member could re-verify a *different*
Epitech address at will. The handler now refuses once
`email_verified` is true.

### The rank ladder has one gate per rung

| Transition | Gated on |
|---|---|
| `Pending` → `Visitor` | Verified `@epitech.eu` address (OTP) |
| `Visitor` → `Initiate` | Joining a first track |
| `Initiate` → `Apprentice` and above | XP alone, via `GlobalRank::from_xp` |

XP **cannot** move a `Pending` or `Visitor` member: `next_rank` returns
their current rank unchanged however much they accumulate. That is what
makes onboarding unskippable rather than merely encouraged.
