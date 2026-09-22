# GameCloud OS — Database Schema

PostgreSQL schema reference, derived from the migrations in
`/migrations/`. Every table is listed with its columns, Rust mirror
type, indexes, and CHECK constraints.

## Conventions

- All primary keys are `UUID` with `DEFAULT gen_random_uuid()` (pgcrypto).
- All timestamps are `TIMESTAMPTZ` (UTC); the corresponding Rust type
  is `chrono::DateTime<Utc>`, aliased as `Timestamp` in
  `gamecloud_shared::models`.
- Track values across all tables share the same allowed list:
  `Engineering`, `GameDesign`, `Narrative`, `VisualArt`, `Audio`,
  `Production`, `QA`, `Marketing`. The application uses
  `gamecloud_shared::roles::Track` as the typed counterpart.
- Email columns use `CITEXT` (case-insensitive) and are validated by
  the regex `^[a-z]+\.[a-z]+@epitech\.eu$`.
- "FK CASCADE" means `ON DELETE CASCADE`; "FK RESTRICT" means
  `ON DELETE RESTRICT`; "FK SET NULL" means `ON DELETE SET NULL`.

## Entity-relationship overview

```
                            ┌─────────────┐
                            │    users    │
                            └──────┬──────┘
                                   │ 1
        ┌──────────────────┬───────┼─────────────┬──────────────┐
        │                  │       │             │              │
        │ N                │ N     │ N           │ N            │ N
   ┌────▼────┐    ┌────────▼───┐ ┌─▼──────┐ ┌────▼─────┐ ┌──────▼──────┐
   │email_   │    │refresh_    │ │track_  │ │special_  │ │   xp_logs   │
   │ otps    │    │ tokens     │ │members │ │ badges   │ │             │
   └─────────┘    └────────────┘ └────────┘ └──────────┘ └─────────────┘
        │
        │ N (created_by, uploaded_by, reviewed_by …)
        ▼
   ┌─────────┐ 1     N ┌────────────────────┐
   │projects │◄────────┤project_contributors│
   │         │         └────────────────────┘
   │         │ 1     N ┌────────────────────┐
   │         │◄────────┤   project_files    │
   │         │         └────────────────────┘
   │         │ 1     N ┌────────────────────┐
   │         │◄────────┤ track_validations  │
   │         │         └────────────────────┘
   │         │ 1     1 ┌────────────────────┐
   │         │◄────────┤   hall_of_fame     │
   └─────────┘         └────────────────────┘

   ┌──────────┐  ┌────────────┐  ┌──────────┐  ┌─────────┐  ┌──────────────┐
   │resources │  │ attendance │  │qr_tokens │  │ roadmap │  │  audit_logs  │
   └──────────┘  └────────────┘  └──────────┘  └─────────┘  └──────────────┘

   ┌────────┐ 1     N ┌───────────────────┐
   │ quests │◄────────┤ quest_completions │
   └────────┘         └───────────────────┘
```

## Migration layout

| File                                | Purpose                                |
| ----------------------------------- | -------------------------------------- |
| `0001_init.sql`                     | Extensions, helper functions           |
| `0002_users.sql`                    | `users`, `email_otps`, `refresh_tokens`|
| `0003_tracks_and_badges.sql`        | `track_memberships`, `special_badges`  |
| `0004_projects.sql`                 | `projects`, contributors, files, validations |
| `0005_resources_attendance.sql`     | `resources`, `attendance`, `qr_tokens` |
| `0006_progression.sql`              | `roadmap`, `quests`, XP/audit logs, hall of fame |

---

## Table: `users`

Mirror: `UserRecord` in `crates/shared/src/models.rs`.

| Column              | Type          | Default              | Notes                              |
| ------------------- | ------------- | -------------------- | ---------------------------------- |
| `id`                | `UUID`        | `gen_random_uuid()`  | PK                                 |
| `discord_id`        | `TEXT`        |                      | UNIQUE, snowflake stored as text   |
| `github_username`   | `TEXT`        |                      | nullable, set during onboarding    |
| `email`             | `CITEXT`      |                      | UNIQUE, nullable until OTP submit  |
| `email_verified`    | `BOOLEAN`     | `FALSE`              |                                    |
| `avatar_url`        | `TEXT`        |                      | Discord avatar (refreshed on login)|
| `avatar_custom_url` | `TEXT`        |                      | Supabase Storage path              |
| `xp_total`          | `BIGINT`      | `0`                  | global XP                          |
| `level`             | `INTEGER`     | `1`                  | cosmetic, derived from XP          |
| `global_rank`       | `TEXT`        | `'Pending'`          | matches `GlobalRank::as_str`       |
| `bureau_role`       | `TEXT`        |                      | nullable                           |
| `current_title`     | `TEXT`        |                      | display title selected by user     |
| `streak_days`       | `INTEGER`     | `0`                  |                                    |
| `last_activity_at`  | `TIMESTAMPTZ` |                      | nullable                           |
| `created_at`        | `TIMESTAMPTZ` | `NOW()`              |                                    |

**CHECK constraints**

- `users_email_epitech_format` — `email` matches `^[a-z]+\.[a-z]+@epitech\.eu$` when present.
- `users_email_verified_consistency` — `email_verified` ⇒ `email IS NOT NULL`.
- `users_xp_total_nonneg` — `xp_total >= 0`.
- `users_streak_nonneg` — `streak_days >= 0`.
- `users_global_rank_valid` — `global_rank` ∈ {`Pending`, `Visitor`, `Initiate`,
  `Apprentice`, `JuniorDev`, `SeniorDev`, `Expert`, `Veteran`, `Legend`, `Myth`}.
- `users_pending_implies_unverified` — `Pending` ⇒ `email_verified = false`.
- `users_above_visitor_requires_verification` — rank above `Visitor`
  ⇒ `email_verified = true`. Defensive duplicate of the type-state
  invariant.

**Indexes**

| Name                          | Columns                                  |
| ----------------------------- | ---------------------------------------- |
| `users_xp_total_desc_idx`     | `(xp_total DESC)`                        |
| `users_global_rank_idx`       | `(global_rank)`                          |
| `users_last_activity_idx`     | `(last_activity_at DESC NULLS LAST)`     |
| `users_bureau_role_idx`       | `(bureau_role)` partial WHERE NOT NULL   |
| `users_github_username_idx`   | `(github_username)` partial WHERE NOT NULL |

---

## Table: `email_otps`

Mirror: `EmailOtpRecord`.

One outstanding code per user (UNIQUE `(user_id)`). The plaintext
6-digit code is **never** persisted; only its Argon2 hash is.

| Column       | Type          | Default              | Notes                          |
| ------------ | ------------- | -------------------- | ------------------------------ |
| `id`         | `UUID`        | `gen_random_uuid()`  | PK                             |
| `user_id`    | `UUID`        |                      | FK CASCADE → `users(id)`       |
| `email`      | `CITEXT`      |                      |                                |
| `code_hash`  | `TEXT`        |                      | Argon2 hash of 6-digit code    |
| `expires_at` | `TIMESTAMPTZ` |                      | typically `created_at + 15min` |
| `attempts`   | `INTEGER`     | `0`                  | 0..5, throttled at 5           |
| `created_at` | `TIMESTAMPTZ` | `NOW()`              |                                |

**CHECK constraints**

- `email_otps_email_format` — Epitech regex.
- `email_otps_attempts_nonneg` — `attempts >= 0`.

**Indexes**

| Name                          | Columns        | Note     |
| ----------------------------- | -------------- | -------- |
| `email_otps_one_per_user_idx` | `(user_id)`    | UNIQUE   |
| `email_otps_expires_idx`      | `(expires_at)` |          |

---

## Table: `refresh_tokens`

Mirror: `RefreshTokenRecord`.

Append-only. Rows are never deleted on rotation — they are marked
`revoked = true` so reuse can be detected.

| Column       | Type          | Default              | Notes                                 |
| ------------ | ------------- | -------------------- | ------------------------------------- |
| `id`         | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `user_id`    | `UUID`        |                      | FK CASCADE → `users(id)`              |
| `token_hash` | `TEXT`        |                      | UNIQUE, SHA-256 of the actual token   |
| `expires_at` | `TIMESTAMPTZ` |                      | typically `created_at + 7d`           |
| `revoked`    | `BOOLEAN`     | `FALSE`              |                                       |
| `user_agent` | `TEXT`        |                      | for "active sessions" UI              |
| `ip_address` | `INET`        |                      |                                       |
| `created_at` | `TIMESTAMPTZ` | `NOW()`              |                                       |

**Indexes**

| Name                          | Columns                          | Note                  |
| ----------------------------- | -------------------------------- | --------------------- |
| `refresh_tokens_user_idx`     | `(user_id)`                      | partial: `revoked=FALSE` |
| `refresh_tokens_expires_idx`  | `(expires_at)`                   | partial: `revoked=FALSE` |

---

## Table: `track_memberships`

Mirror: `TrackMembershipRecord`.

One row per `(user_id, track)` pair.

| Column            | Type          | Default              | Notes                                 |
| ----------------- | ------------- | -------------------- | ------------------------------------- |
| `id`              | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `user_id`         | `UUID`        |                      | FK CASCADE → `users(id)`              |
| `track`           | `TEXT`        |                      | one of the 8 allowed values           |
| `specialization`  | `TEXT`        |                      | nullable, free-form (validated app-side) |
| `track_role`      | `TEXT`        | `'Observer'`         | matches `TrackRole::as_str`           |
| `track_xp`        | `BIGINT`      | `0`                  |                                       |
| `joined_at`       | `TIMESTAMPTZ` | `NOW()`              |                                       |
| `last_active_at`  | `TIMESTAMPTZ` |                      | last project submission, etc.         |

**CHECK constraints**

- `track_memberships_track_valid` — track in allowed list.
- `track_memberships_role_valid` — `track_role` ∈ {`Observer`,
  `Contributor`, `Reviewer`, `Mentor`, `CoLead`, `Lead`}.
- `track_memberships_xp_nonneg` — `track_xp >= 0`.

**Indexes**

| Name                                   | Columns                              | Note                            |
| -------------------------------------- | ------------------------------------ | ------------------------------- |
| `track_memberships_user_track_idx`     | `(user_id, track)`                   | UNIQUE                          |
| `track_memberships_track_xp_idx`       | `(track, track_xp DESC)`             | leaderboard per track           |
| `track_memberships_leads_idx`          | `(track, track_role)`                | partial, role IN (`Lead`,`CoLead`) |

---

## Table: `special_badges`

Mirror: `SpecialBadgeRecord`.

| Column        | Type          | Default              | Notes                                 |
| ------------- | ------------- | -------------------- | ------------------------------------- |
| `id`          | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `user_id`     | `UUID`        |                      | FK CASCADE → `users(id)`              |
| `badge_type`  | `TEXT`        |                      | matches `SpecialBadge` string form    |
| `awarded_at`  | `TIMESTAMPTZ` | `NOW()`              |                                       |
| `awarded_by`  | `UUID`        |                      | FK SET NULL → `users(id)`, nullable   |

**CHECK constraints**

- `special_badges_type_valid` — `badge_type` ∈ 15 allowed values
  (matches `gamecloud_shared::roles::SpecialBadge`).

**Indexes**

| Name                          | Columns               | Note     |
| ----------------------------- | --------------------- | -------- |
| `special_badges_user_type_idx`| `(user_id, badge_type)` | UNIQUE |
| `special_badges_user_idx`     | `(user_id)`           |          |

---

## Table: `projects`

Mirror: `ProjectRecord`.

| Column                | Type          | Default              | Notes                                  |
| --------------------- | ------------- | -------------------- | -------------------------------------- |
| `id`                  | `UUID`        | `gen_random_uuid()`  | PK                                     |
| `name`                | `TEXT`        |                      |                                        |
| `short_description`   | `TEXT`        |                      |                                        |
| `long_description`    | `TEXT`        |                      | markdown                               |
| `primary_track`       | `TEXT`        |                      | drives accent color in UI              |
| `status`              | `TEXT`        | `'Draft'`            | matches `ProjectStatus`                |
| `block_number`        | `INTEGER`     |                      | 1..=8 when present                     |
| `epitech_level`       | `TEXT`        |                      | `Tek1`/`Tek2`/`Tek3`/`Master`          |
| `rarity`              | `TEXT`        | `'Common'`           | matches `Rarity`                       |
| `thumbnail_url`       | `TEXT`        |                      |                                        |
| `banner_url`          | `TEXT`        |                      |                                        |
| `screenshots`         | `TEXT[]`      | `'{}'`               | up to 10                               |
| `video_url`           | `TEXT`        |                      |                                        |
| `gifs`                | `TEXT[]`      | `'{}'`               |                                        |
| `github_repo_url`     | `TEXT`        |                      |                                        |
| `itch_url`            | `TEXT`        |                      |                                        |
| `created_by`          | `UUID`        |                      | FK RESTRICT → `users(id)`              |
| `created_at`          | `TIMESTAMPTZ` | `NOW()`              |                                        |
| `released_at`         | `TIMESTAMPTZ` |                      | required when status is Released/Archived |

**CHECK constraints**

- `projects_primary_track_valid` — track in allowed list.
- `projects_status_valid` — status in {`Draft`, `InReview`, `PartialOK`,
  `Approved`, `Released`, `Archived`, `Rejected`}.
- `projects_rarity_valid` — `Common`, `Rare`, `Epic`, `Legendary`, `Mythic`.
- `projects_block_range` — `block_number BETWEEN 1 AND 8` if present.
- `projects_epitech_level_valid` — level in allowed set.
- `projects_released_consistency` — status `Released`/`Archived`
  requires `released_at` non-null.
- `projects_screenshots_max` — `cardinality(screenshots) <= 10`.

**Indexes**

| Name                          | Columns                             | Note                       |
| ----------------------------- | ----------------------------------- | -------------------------- |
| `projects_status_idx`         | `(status)`                          |                            |
| `projects_primary_track_idx`  | `(primary_track)`                   |                            |
| `projects_block_idx`          | `(block_number)`                    | partial: NOT NULL          |
| `projects_released_at_idx`    | `(released_at DESC NULLS LAST)`     | partial: status `Released` |
| `projects_creator_idx`        | `(created_by)`                      |                            |

The legal status transition matrix is enforced in application code by
`ProjectStatus::can_transition_to` (see `gamecloud_shared::models`).

---

## Table: `project_contributors`

Mirror: `ProjectContributorRecord`. Composite PK `(project_id, user_id, track)`.

| Column           | Type   | Notes                                          |
| ---------------- | ------ | ---------------------------------------------- |
| `project_id`     | `UUID` | FK CASCADE → `projects(id)` (PK component)     |
| `user_id`        | `UUID` | FK CASCADE → `users(id)` (PK component)        |
| `track`          | `TEXT` | track this contributor represents (PK component) |
| `role_in_project`| `TEXT` | free-form ("Lead Programmer", "Composer"…)     |

**CHECK**: `project_contributors_track_valid`.

**Indexes**: `project_contributors_user_idx (user_id)`,
`project_contributors_track_idx (track)`.

---

## Table: `project_files`

Mirror: `ProjectFileRecord`.

| Column              | Type          | Default              | Notes                                 |
| ------------------- | ------------- | -------------------- | ------------------------------------- |
| `id`                | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `project_id`        | `UUID`        |                      | FK CASCADE → `projects(id)`           |
| `filename`          | `TEXT`        |                      |                                       |
| `file_type`         | `TEXT`        |                      | `Executable`/`Source`/`Asset`/`Doc`/`Audio`/`Video` |
| `size_bytes`        | `BIGINT`      |                      | ≤ 524 288 000 (500 MB)                |
| `checksum_sha256`   | `TEXT`        |                      | regex `^[0-9a-f]{64}$`                |
| `storage_path`      | `TEXT`        |                      | Supabase path                         |
| `version`           | `TEXT`        | `'v1.0'`             |                                       |
| `changelog`         | `TEXT`        |                      |                                       |
| `uploaded_by`       | `UUID`        |                      | FK RESTRICT → `users(id)`             |
| `uploaded_at`       | `TIMESTAMPTZ` | `NOW()`              |                                       |

**CHECK constraints**

- `project_files_type_valid`.
- `project_files_size_positive` — `size_bytes > 0`.
- `project_files_size_max` — `size_bytes <= 524288000`.
- `project_files_checksum_format` — 64 lowercase hex characters.

**Index**: `project_files_project_idx (project_id)`.

---

## Table: `track_validations`

Mirror: `TrackValidationRecord`.

One row per `(project_id, track)` pair, created when a project moves
from `Draft` to `InReview`.

| Column         | Type          | Default              | Notes                                 |
| -------------- | ------------- | -------------------- | ------------------------------------- |
| `id`           | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `project_id`   | `UUID`        |                      | FK CASCADE → `projects(id)`           |
| `track`        | `TEXT`        |                      |                                       |
| `status`       | `TEXT`        | `'Pending'`          | `Pending`/`Approved`/`Rejected`/`NotApplicable` |
| `reviewed_by`  | `UUID`        |                      | FK SET NULL → `users(id)`             |
| `feedback`     | `TEXT`        |                      | required when status = `Rejected`     |
| `reviewed_at`  | `TIMESTAMPTZ` |                      | required when status ≠ `Pending`      |

**CHECK constraints**

- `track_validations_track_valid`.
- `track_validations_status_valid`.
- `track_validations_reject_requires_feedback` — `Rejected`
  ⇒ `feedback IS NOT NULL AND length(feedback) > 0`.
- `track_validations_review_consistency` — non-`Pending`
  ⇒ `reviewed_by` and `reviewed_at` both non-null.

**Indexes**

| Name                                   | Columns                             | Note                  |
| -------------------------------------- | ----------------------------------- | --------------------- |
| `track_validations_project_track_idx`  | `(project_id, track)`               | UNIQUE                |
| `track_validations_pending_idx`        | `(track)`                           | partial: `Pending`    |

---

## Table: `resources`

Mirror: `ResourceRecord`.

| Column            | Type          | Default              | Notes                                |
| ----------------- | ------------- | -------------------- | ------------------------------------ |
| `id`              | `UUID`        | `gen_random_uuid()`  | PK                                   |
| `title`           | `TEXT`        |                      |                                      |
| `url`             | `TEXT`        |                      | must match `^https?://`              |
| `resource_type`   | `TEXT`        |                      | `Tutorial`/`Tool`/`Asset`/`Doc`/`Video` |
| `tracks`          | `TEXT[]`      | `'{}'`               | GIN-indexed                          |
| `specializations` | `TEXT[]`      | `'{}'`               |                                      |
| `level`           | `TEXT`        |                      | `Initiate`/`Junior`/`Senior`/`Expert`|
| `submitted_by`    | `UUID`        |                      | FK RESTRICT → `users(id)`            |
| `validated_by`    | `UUID`        |                      | FK SET NULL → `users(id)`, nullable  |
| `votes`           | `INTEGER`     | `0`                  | net upvotes                          |
| `is_official`     | `BOOLEAN`     | `FALSE`              | curated by Bureau                    |
| `created_at`      | `TIMESTAMPTZ` | `NOW()`              |                                      |

**CHECK**: `resources_type_valid`, `resources_level_valid`,
`resources_url_format`.

**Indexes**

| Name                       | Columns                      | Note                       |
| -------------------------- | ---------------------------- | -------------------------- |
| `resources_validated_idx`  | `(validated_by)`             | partial: NOT NULL          |
| `resources_tracks_gin`     | `tracks`                     | GIN — fast `&&` queries    |
| `resources_votes_idx`      | `(votes DESC)`               |                            |

---

## Table: `attendance`

Mirror: `AttendanceRecord`.

| Column         | Type          | Default              | Notes                              |
| -------------- | ------------- | -------------------- | ---------------------------------- |
| `id`           | `UUID`        | `gen_random_uuid()`  | PK                                 |
| `user_id`      | `UUID`        |                      | FK CASCADE → `users(id)`           |
| `event_name`   | `TEXT`        |                      |                                    |
| `event_type`   | `TEXT`        |                      | `Session`/`OfficeHours`/`StandUp`/`GameJam`/`Special` |
| `xp_rewarded`  | `INTEGER`     |                      | ≥ 0                                |
| `scanned_at`   | `TIMESTAMPTZ` | `NOW()`              |                                    |

**CHECK**: `attendance_type_valid`, `attendance_xp_nonneg`.

**Indexes**: `attendance_user_idx (user_id, scanned_at DESC)`,
`attendance_event_idx (event_name)`,
`attendance_scanned_idx (scanned_at DESC)`.

---

## Table: `qr_tokens`

Mirror: `QrTokenRecord`.

The `token` column stores the **JWT itself** (used as the QR payload);
`id` is the internal PK.

| Column        | Type          | Default              | Notes                              |
| ------------- | ------------- | -------------------- | ---------------------------------- |
| `id`          | `UUID`        | `gen_random_uuid()`  | PK                                 |
| `token`       | `TEXT`        |                      | UNIQUE, the JWT string             |
| `event_name`  | `TEXT`        |                      |                                    |
| `event_type`  | `TEXT`        |                      | same enum as `attendance.event_type` |
| `xp_value`    | `INTEGER`     |                      | ≥ 0                                |
| `created_by`  | `UUID`        |                      | FK RESTRICT → `users(id)`          |
| `expires_at`  | `TIMESTAMPTZ` |                      | also encoded in JWT `exp` claim    |
| `is_used`     | `BOOLEAN`     | `FALSE`              |                                    |

**CHECK**: `qr_tokens_type_valid`, `qr_tokens_xp_nonneg`.

**Index**: `qr_tokens_active_idx (expires_at)` — partial WHERE
`is_used = FALSE`.

The scan flow uses `SELECT … FOR UPDATE` on `qr_tokens` and atomically
sets `is_used = TRUE` to prevent the double-scan race.

---

## Table: `roadmap`

Mirror: `RoadmapEntryRecord`.

| Column           | Type          | Default              | Notes                                 |
| ---------------- | ------------- | -------------------- | ------------------------------------- |
| `id`             | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `milestone_name` | `TEXT`        |                      |                                       |
| `description`    | `TEXT`        |                      |                                       |
| `target_date`    | `DATE`        |                      | nullable                              |
| `status`         | `TEXT`        | `'Todo'`             | `Todo`/`InProgress`/`Completed`       |
| `block_number`   | `INTEGER`     |                      | 1..=8                                 |
| `is_major_boss`  | `BOOLEAN`     | `FALSE`              | major boss of its block               |
| `track`          | `TEXT`        |                      | nullable for transversal entries      |
| `completed_at`   | `TIMESTAMPTZ` |                      | required when status `Completed`      |

**CHECK**: `roadmap_status_valid`, `roadmap_block_range`,
`roadmap_track_valid`,
`roadmap_completed_consistency` (status `Completed` ↔ `completed_at`).

**Indexes**: `roadmap_status_idx`, `roadmap_block_idx` (partial).

---

## Table: `quests`

Mirror: `QuestRecord`.

| Column            | Type          | Default              | Notes                                |
| ----------------- | ------------- | -------------------- | ------------------------------------ |
| `id`              | `UUID`        | `gen_random_uuid()`  | PK                                   |
| `title`           | `TEXT`        |                      |                                      |
| `description`     | `TEXT`        |                      |                                      |
| `xp_reward`       | `INTEGER`     |                      | > 0                                  |
| `quest_type`      | `TEXT`        |                      | `Weekly`/`Special`/`Hidden`          |
| `condition_type`  | `TEXT`        |                      | `Push`/`Attend`/`Submit`/`Review`    |
| `condition_value` | `INTEGER`     |                      | > 0                                  |
| `track`           | `TEXT`        |                      | nullable for global quests           |
| `starts_at`       | `TIMESTAMPTZ` |                      |                                      |
| `ends_at`         | `TIMESTAMPTZ` |                      | > `starts_at`                        |
| `created_by`      | `UUID`        |                      | FK RESTRICT → `users(id)`            |

**CHECK**: `quests_type_valid`, `quests_condition_valid`,
`quests_track_valid`, `quests_xp_positive`,
`quests_condition_positive`, `quests_window_valid`.

**Index**: `quests_active_idx (starts_at, ends_at)`.

---

## Table: `quest_completions`

Mirror: `QuestCompletionRecord`. Composite PK `(quest_id, user_id)`.

| Column         | Type          | Default              | Notes                              |
| -------------- | ------------- | -------------------- | ---------------------------------- |
| `quest_id`     | `UUID`        |                      | FK CASCADE → `quests(id)`          |
| `user_id`      | `UUID`        |                      | FK CASCADE → `users(id)`           |
| `completed_at` | `TIMESTAMPTZ` | `NOW()`              |                                    |

**Index**: `quest_completions_user_idx (user_id)`.

---

## Table: `xp_logs`

Mirror: `XpLogRecord`.

Append-only ledger. Every write to `users.xp_total` and
`track_memberships.track_xp` has a matching row here, written in the
same transaction.

| Column        | Type          | Default              | Notes                                  |
| ------------- | ------------- | -------------------- | -------------------------------------- |
| `id`          | `UUID`        | `gen_random_uuid()`  | PK                                     |
| `user_id`     | `UUID`        |                      | FK CASCADE → `users(id)`               |
| `amount`      | `INTEGER`     |                      | may be negative for manual revoke      |
| `source`      | `TEXT`        |                      | `GitHub`/`QR`/`Discord`/`Review`/`Quest`/`Manual`/`Project` |
| `track`       | `TEXT`        |                      | nullable for global-only XP            |
| `description` | `TEXT`        |                      | activity-feed text                     |
| `created_at`  | `TIMESTAMPTZ` | `NOW()`              |                                        |

**CHECK**: `xp_logs_source_valid`, `xp_logs_track_valid`.

> `amount` is intentionally **not** constrained to be positive: a
> manual revoke records a negative amount with `source = 'Manual'`.

**Indexes**

| Name                            | Columns                                                | Note            |
| ------------------------------- | ------------------------------------------------------ | --------------- |
| `xp_logs_user_created_idx`      | `(user_id, created_at DESC)`                           |                 |
| `xp_logs_source_idx`            | `(source, created_at DESC)`                            |                 |
| `xp_logs_user_source_day_idx`   | `(user_id, source, date_trunc('day', created_at))`     | daily-cap query |

---

## Table: `audit_logs`

Mirror: `AuditLogRecord`. Append-only.

| Column        | Type          | Default              | Notes                                  |
| ------------- | ------------- | -------------------- | -------------------------------------- |
| `id`          | `UUID`        | `gen_random_uuid()`  | PK                                     |
| `actor_id`    | `UUID`        |                      | FK RESTRICT → `users(id)`              |
| `action`      | `TEXT`        |                      | e.g. `ProjectReleased`, `BureauRoleAssigned` |
| `target_type` | `TEXT`        |                      | `User`/`Project`/`Role`/`Resource`     |
| `target_id`   | `UUID`        |                      | nullable                               |
| `metadata`    | `JSONB`       |                      | free-form, GIN-indexed                 |
| `created_at`  | `TIMESTAMPTZ` | `NOW()`              |                                        |

**Indexes**: `audit_logs_actor_idx (actor_id, created_at DESC)`,
`audit_logs_action_idx`, `audit_logs_target_idx (target_type, target_id)`,
`audit_logs_metadata_gin (metadata)` (GIN).

---

## Table: `hall_of_fame`

Mirror: `HallOfFameEntryRecord`. Auto-generated when a project moves
to `Released`.

| Column                  | Type          | Default              | Notes                                 |
| ----------------------- | ------------- | -------------------- | ------------------------------------- |
| `id`                    | `UUID`        | `gen_random_uuid()`  | PK                                    |
| `project_id`            | `UUID`        |                      | UNIQUE, FK CASCADE → `projects(id)`   |
| `featured_at`           | `TIMESTAMPTZ` | `NOW()`              |                                       |
| `total_xp_distributed`  | `INTEGER`     | `0`                  | sum of XP awarded to contributors     |
| `boss_defeated_by`      | `UUID[]`      | `'{}'`               | denormalized contributors list        |

**CHECK**: `hall_of_fame_xp_nonneg`.

**Index**: `hall_of_fame_featured_idx (featured_at DESC)`.

---

## Allowed enum values reference

Quick lookup of every string-enum used in the schema. Each pairs with a
typed Rust enum in `gamecloud_shared`.

| Schema column                       | Allowed values                                                                      | Rust type           |
| ----------------------------------- | ----------------------------------------------------------------------------------- | ------------------- |
| `users.global_rank`                 | `Pending`, `Visitor`, `Initiate`, `Apprentice`, `JuniorDev`, `SeniorDev`, `Expert`, `Veteran`, `Legend`, `Myth` | `GlobalRank`        |
| `users.bureau_role`                 | 16 values (see `BureauRole`)                                                        | `BureauRole`        |
| `track_memberships.track_role`      | `Observer`, `Contributor`, `Reviewer`, `Mentor`, `CoLead`, `Lead`                  | `TrackRole`         |
| `*.track`                           | `Engineering`, `GameDesign`, `Narrative`, `VisualArt`, `Audio`, `Production`, `QA`, `Marketing` | `Track`             |
| `projects.status`                   | `Draft`, `InReview`, `PartialOK`, `Approved`, `Released`, `Archived`, `Rejected`   | `ProjectStatus`     |
| `projects.rarity`                   | `Common`, `Rare`, `Epic`, `Legendary`, `Mythic`                                     | `Rarity`            |
| `projects.epitech_level`            | `Tek1`, `Tek2`, `Tek3`, `Master`                                                    | `EpitechLevel`      |
| `track_validations.status`          | `Pending`, `Approved`, `Rejected`, `NotApplicable`                                  | (string in handler) |
| `attendance.event_type`, `qr_tokens.event_type` | `Session`, `OfficeHours`, `StandUp`, `GameJam`, `Special`             | (string in handler) |
| `resources.resource_type`           | `Tutorial`, `Tool`, `Asset`, `Doc`, `Video`                                         | (string in handler) |
| `resources.level`                   | `Initiate`, `Junior`, `Senior`, `Expert`                                            | (string in handler) |
| `roadmap.status`                    | `Todo`, `InProgress`, `Completed`                                                   | (string in handler) |
| `quests.quest_type`                 | `Weekly`, `Special`, `Hidden`                                                       | (string in handler) |
| `quests.condition_type`             | `Push`, `Attend`, `Submit`, `Review`                                                | (string in handler) |
| `xp_logs.source`                    | `GitHub`, `QR`, `Discord`, `Review`, `Quest`, `Manual`, `Project`                  | `XpSource`          |
| `special_badges.badge_type`         | 15 values (see `SpecialBadge`)                                                      | `SpecialBadge`      |

## Maintenance & operations

- **Migrations** run via `sqlx migrate run`. The `_sqlx_migrations`
  table is created automatically on first run.
- **Backups** — point-in-time recovery enabled at the managed-Postgres
  level (target: 7 days). Logical dumps nightly via `pg_dump`.
- **Reseeding for tests** — phase 2 will introduce an `xtask` binary
  with a `reset-db` subcommand: drop schema, run all migrations, seed
  with a deterministic fixture set.
- **Foreign-key strategy** — `RESTRICT` on `created_by`/`uploaded_by`
  columns is intentional: a user with content cannot be hard-deleted.
  Either anonymize their record or `Alumni`-tag them.

## Relation to the type-state model

The schema is **defensively** consistent: `CHECK` constraints duplicate
some invariants enforced in code (e.g. "users above `Visitor` must have
`email_verified = true`"). This is on purpose. Application code is the
fast path; the database is the slow, authoritative last line of defense
against bugs in any layer.

The bidirectional contract is:

1. `gamecloud_shared::models::UserRecord` exposes `classify()` which
   reads `email` and `email_verified` and returns the correct
   `User<Pending>` / `User<EmailSubmitted>` / `User<Verified>` typed
   wrapper. XP-granting functions accept only `User<Verified>` at
   compile time.
2. The `users_above_visitor_requires_verification` CHECK fires if any
   future code path were ever to grant XP without going through the
   typed wrapper. The two checks together are belt + suspenders.

---

# Migrations 0009–0011 (September 2026)

## 0009 — Security hardening

### `email_otps` — three new columns

| Column | Type | Why |
|---|---|---|
| `cumulative_attempts` | `INTEGER NOT NULL DEFAULT 0` | Failed guesses across **every** code issued to this member. `attempts` counts only the current code and is reset on each resend, which is exactly the hole this closes: five guesses, request a new code, five more, forever. The application enforces its lockout against this column. |
| `resend_count` | `INTEGER NOT NULL DEFAULT 0` | Codes requested so far. Capped at 5. |
| `last_sent_at` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` | Drives the 60-second resend cooldown. |

`upsert_otp` is now an `ON CONFLICT DO UPDATE` that deliberately carries
`cumulative_attempts` forward rather than a `DELETE` + `INSERT`.

### `qr_tokens` — hashed, countable

| Column | Type | Why |
|---|---|---|
| `token_hash` | `TEXT NOT NULL`, unique | SHA-256 of the JWT. The plaintext `token` column is **deprecated and no longer written** — a read of this table used to hand out working bearer tokens. |
| `max_scans` | `INTEGER` | Optional ceiling. `NULL` = unlimited until expiry. |
| `scan_count` | `INTEGER NOT NULL DEFAULT 0` | How many members have claimed it. |

`is_used` is **deprecated**. It was set by the first scan, which meant
one member got the XP for a session and everybody else got a 410.

### `attendance` — per-member claims

| Column | Type | Why |
|---|---|---|
| `qr_token_id` | `UUID REFERENCES qr_tokens(id) ON DELETE SET NULL` | Which token this attendance came from. |

```sql
CREATE UNIQUE INDEX attendance_user_token_idx
    ON attendance (user_id, qr_token_id)
    WHERE qr_token_id IS NOT NULL;
```

This index is the replay defence that `is_used` used to provide, without
preventing everybody else at the event from scanning the same code.

## 0010 — Seasons and streaks

### `users` — streak state

| Column | Type | Why |
|---|---|---|
| `last_streak_day` | `DATE` | The last UTC day on which this member earned XP. Distinguishing "yesterday" (continue) from "today" (no-op) from "older" (reset) needs exactly this one extra piece of state. |
| `longest_streak` | `INTEGER NOT NULL DEFAULT 0` | Personal best, for the record and the `Streaker` badge. |

Before this migration `streak_days` was read by the XP multiplier and
displayed by the bot, but **no code ever wrote it** — so the multiplier
always evaluated to 1.0.

### `seasons`

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `name` | `TEXT NOT NULL` | e.g. "Saison 1 — La Forge" |
| `slug` | `TEXT NOT NULL UNIQUE` | `^[a-z0-9][a-z0-9-]*$` |
| `description` | `TEXT` | |
| `starts_at` / `ends_at` | `TIMESTAMPTZ NOT NULL` | `ends_at > starts_at` |

```sql
ALTER TABLE seasons ADD CONSTRAINT seasons_no_overlap
    EXCLUDE USING gist (tstzrange(starts_at, ends_at) WITH &&);
```

At most one season may be open at any instant, so "the current season"
is never ambiguous. Requires the `btree_gist` extension.

The migration seeds one year-long season so the feature is live on first
boot. The Bureau closes it early by editing `ends_at`; the exclusion
constraint then allows the next one to start.

### `xp_logs.season_id`

`UUID REFERENCES seasons(id) ON DELETE SET NULL`. Every XP event is
stamped with the season open when it happened. `NULL` means "earned
outside any season" and simply never shows on a season board.

## 0011 — Engagement

### `resource_votes`

| Column | Type |
|---|---|
| `resource_id` | `UUID` → `resources(id)` |
| `user_id` | `UUID` → `users(id)` |
| `created_at` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` |

Primary key `(resource_id, user_id)` — one vote per member.
`resources.votes` becomes a cached aggregate maintained by the
`resource_votes_sync` trigger; it used to be a bare counter with nothing
stopping a thousand clicks.

### `quest_progress`

| Column | Type |
|---|---|
| `quest_id` | `UUID` → `quests(id)` |
| `user_id` | `UUID` → `users(id)` |
| `counter` | `INTEGER NOT NULL DEFAULT 0`, non-negative |
| `updated_at` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` |

`quest_completions` recorded the finish line but not the run-up, so a
member could never see "2 / 3 pushes done".

### `audit_logs.actor_id` is now nullable

`NULL` means the platform acted on its own — a webhook grant, a
scheduled job. It was `NOT NULL`, which made system-originated entries
impossible to record, which is part of why nothing ever wrote to this
table.

---

# Table usage

Every table is now read or written by code. For orientation:

| Table | Written by |
|---|---|
| `users`, `email_otps`, `refresh_tokens` | auth routes |
| `track_memberships` | `db::queries::tracks` (onboarding) |
| `xp_logs`, `seasons` | `db::queries::xp` |
| `special_badges` | `db::queries::xp` (auto) and `badges` (manual) |
| `quests`, `quest_progress`, `quest_completions` | `db::queries::quests`, `xp` |
| `projects`, `project_contributors`, `track_validations`, `hall_of_fame` | `db::queries::projects` |
| `resources`, `resource_votes` | `db::queries::resources` |
| `qr_tokens`, `attendance` | `db::queries::qr` — both now carry `event_id` |
| `events` | `db::queries::events` (calendar) |
| `audit_logs` | `db::queries::audit` |
| `notifications_outbox` | `services::notifications` (web) → drained by the bot |
| `project_files` | `db::queries::files` — builds live in a GitHub release, this table indexes them |
| `roadmap` | reserved for the block/boss view (not yet built) |


---

# `events` — the calendar

Added in `0015_calendar_and_grading.sql`. Attendance previously hung off
a free-text `event_name` on `qr_tokens`, which meant two weeks of Tuesday
sessions were unrelated strings and no question about attendance over
time had an answer.

| Column | Notes |
|---|---|
| `kind` | `Session`, `Workshop`, `Jam`, `Meeting`, `Deadline`, `Showcase` |
| `track` | `NULL` = the whole association. Also decides who may edit it. |
| `xp_reward` | 0–500. Zero is legitimate: a deadline is seen, not attended. |
| `cancelled_at` | Events are **cancelled, never deleted** — attendance points at them. |

`qr_tokens.event_id` and `attendance.event_id` are both nullable, and
nothing was backfilled: rows written before events existed keep their
free-text name and point at nothing.

A unique index on `(user_id, event_id)` means a member counts once per
event however many codes are printed for it. Without it, an organiser
who reprinted the QR would have paid the same member twice.

# `member_display_name(...)`

Added in `0014_discord_identity.sql`. Resolves a member to
`title → Discord display name → Discord handle → snowflake`. It exists as
a SQL function rather than a `COALESCE` chain because eight queries
needed the same precedence and copies drift apart.

`UserRecord::display_name()` mirrors it in Rust for rows read through
`sqlx::query_as`.

# `track_validations.score`

Added in `0015`. A mark out of 100 for one track's share of a project,
`NULL` when the reviewer only gated. Approving is a gate; grading is a
judgement, and a reviewer who only wants to open the gate is not forced
to invent a number.


# `events.audience`

Added in `0016_event_audience.sql`. `Association`, `Track` or `Bureau`.

`track IS NULL` used to mean "the whole association", which left no way
to express a third and genuinely different case: a Bureau meeting. Those
are not merely unannounced — they must not appear on an ordinary
member's calendar at all, and that is a visibility rule a nullable column
cannot carry.

Two CHECK constraints keep the pair honest: a `Track` event must name a
track, and an `Association` or `Bureau` event must not. `EventScope::parse`
in `gamecloud-shared` mirrors both directions exactly, so a row that
somehow broke its constraint is refused in Rust rather than being read as
public.

Bureau meetings are filtered out **in SQL**, not after the fetch — a row
that reaches the browser and merely goes undrawn is not private.


# `users.offices`

*Migration 0026.* Liste des offices du Bureau tenus, dans l'ordre
protocolaire de `BureauRole::ALL` (président·e d'abord). C'est la seule
colonne d'office qui s'écrit, via `users::change_offices`.

`bureau_role` est désormais `GENERATED ALWAYS AS (offices[1]) STORED` :
l'office principal, en lecture seule, pour ce qui n'affiche qu'un titre.
Un `CHECK` limite les valeurs aux offices connus ; `Provisional`
(« membre du Bureau sans office ») disparaît dès qu'un vrai office est
ajouté.
