# GameCloud OS — API Reference

HTTP API exposed by the `gamecloud-web` Axum binary. All routes return
JSON unless noted.

> Phase 2 covers authentication, QR attendance, and ingress webhooks.
> Project / track / leaderboard / admin routes ship in a later phase
> and will follow the same conventions documented here.

## Conventions

### Cookies

Two cookies are issued at login and rotated on `/api/auth/refresh`:

| Cookie       | Path        | SameSite | Secure*       | HttpOnly | Lifetime |
| ------------ | ----------- | -------- | ------------- | -------- | -------- |
| `gc_access`  | `/`         | Lax      | prod only     | yes      | 1 hour   |
| `gc_refresh` | `/api/auth` | Strict   | prod only     | yes      | 7 days   |

*`Secure` is set when `APP_ENV=production`. The `gc_refresh` cookie is
scoped to `/api/auth` so the browser only sends it on refresh / logout
requests, never on regular API calls.

### Authentication

Every protected route reads `gc_access`, verifies the JWT, and loads
the user record. Without a valid cookie the response is `401
Unauthorized` with body:

```json
{ "error": { "code": "unauthorized", "message": "unauthorized" } }
```

### Error envelope

Every error response, regardless of status code, uses the same shape:

```json
{ "error": { "code": "<machine_code>", "message": "<human_readable>" } }
```

For 5xx responses, `message` is always the literal string
`"internal server error"`; details are logged server-side and never
leaked. For 4xx responses, `message` carries the actionable detail.

| HTTP | code                     | When                                                  |
| ---- | ------------------------ | ----------------------------------------------------- |
| 401  | `unauthorized`           | Missing / invalid / expired credentials               |
| 403  | `forbidden`              | Authenticated but lacking the required role           |
| 403  | `email_not_verified`     | Action requires a verified `@epitech.eu` email        |
| 404  | `not_found`              | Resource does not exist                               |
| 409  | `conflict`               | Write conflict (rare; usually a unique-constraint race) |
| 410  | `invalid_qr_token`       | QR token is unknown, expired, or already consumed     |
| 422  | `invalid_email`          | Email does not match the Epitech format               |
| 422  | `unknown_track`          | Unknown track identifier                              |
| 422  | `unknown_specialization` | Specialization not in the canonical list              |
| 422  | `invalid_transition`     | Project status transition is not allowed              |
| 422  | `validation`             | Body / query is structurally OK but semantically bad  |
| 429  | `daily_xp_cap`           | Daily XP cap reached for this source                  |
| 429  | `rate_limited`           | Rate limit, or the OTP attempt ceiling                 |
| 429  | `otp_cooldown`           | A new OTP was requested too soon after the last        |
| 409  | `email_taken`            | Another account has verified that Epitech address      |
| 409  | `qr_already_claimed`     | You already scanned this code                          |
| 409  | `already_voted`          | You already voted for this resource                    |
| 409  | `already_in_track`       | You already belong to that track                       |
| 410  | `qr_capacity_reached`    | The code hit its scan ceiling                          |
| 403  | `email_already_verified` | The address is verified and cannot be changed here     |
| 500  | `internal`               | Unhandled server error or DB error                    |
| 500  | `invariant`              | Server-side data invariant violated                   |
| 502  | `upstream`               | Discord / GitHub / SMTP failure                       |

---

## Health

### `GET /healthz`

Liveness probe. Always returns `200 OK` with body `ok` if the process
can answer.

### `GET /readyz`

Readiness probe. Verifies the database is reachable.

```json
{ "db": true }
```

`200 OK` when ready, `503 Service Unavailable` when not.

---

## Authentication

### `GET /api/auth/login`

Starts the Discord OAuth dance.

- Sets a 10-minute `gc_oauth_state` cookie carrying a CSRF random
  token.
- Responds with `302 Found`, `Location: <Discord authorize URL>`.

The Discord authorize URL is built from `DISCORD_CLIENT_ID`,
`DISCORD_REDIRECT_URI` and the random state.

### `GET /api/auth/callback?code=…&state=…`

Discord redirect target.

1. Verifies `state` matches the cookie set by `/login`.
2. Exchanges `code` for a Discord access token.
3. Fetches `https://discord.com/api/users/@me`.
4. UPSERTs a `users` row keyed on `discord_id`.
5. Issues `gc_access` and `gc_refresh` cookies.
6. Deletes `gc_oauth_state`.
7. Responds with `302 Found` to a destination that depends on the
   user's stage:
   - **Verified** (`email_verified = true`) → `/profile`
   - **Email submitted** but not verified → `/onboarding/verify`
   - **Pending** (no email) → `/onboarding/email`

Errors:

- `401 unauthorized` — state mismatch or missing cookie.
- `502 upstream` — Discord token exchange or `/me` failure.

### `POST /api/auth/email`

Submit an `@epitech.eu` email and trigger the OTP send.

**Auth**: `gc_access` (any stage).

**Content type**: accepts `application/json` *or*
`application/x-www-form-urlencoded`. The latter lets browser `<form>`
POSTs work without JavaScript.

**Request**:

```json
{ "email": "joachim.goehakue@epitech.eu" }
```

**Response** `200 OK` (JSON request):

```json
{ "ok": true }
```

**Response** `303 See Other` to `/onboarding/verify` (form request).

**Side effects**:

- Validates against `^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*@epitech\.eu$`
  (case-insensitive on input, lowercased on storage). Hyphens and
  digits are allowed inside the name parts.
- Generates a 6-digit OTP, hashes it with Argon2id, and upserts a
  `email_otps` row (one outstanding per user).
- Sends the code via SMTP.

**Errors**:

- `422 invalid_email` — wrong format.
- `502 upstream` — SMTP failure.

### `POST /api/auth/verify`

Submit the OTP code.

**Auth**: `gc_access` (any stage).

**Content type**: accepts `application/json` *or*
`application/x-www-form-urlencoded`.

**Request**:

```json
{ "code": "042973" }
```

**Response** `200 OK` (JSON):

```json
{ "ok": true }
```

**Response** `303 See Other` to `/profile` (form).

**Side effects on success**:

- `users.email = <stored email>`, `email_verified = true`.
- If `global_rank` was `Pending`, it is bumped to `Visitor`.
- The `email_otps` row is deleted.

**Errors**:

- `401 unauthorized` — no outstanding OTP, expired, wrong code.
- `429 rate_limited` — 10 cumulative failed attempts. **This counter
  survives a resend**: requesting a new code no longer resets it, which
  is what closes the brute-force path a 6-digit secret otherwise has.

After 5 failed attempts the user must call `/api/auth/email` again to
get a fresh code; the old one stays burned.

### `POST /api/auth/refresh`

Rotate the access + refresh tokens.

**Auth**: `gc_refresh` cookie only.

**Response** `200 OK`:

```json
{ "ok": true }
```

Sets fresh `gc_access` and `gc_refresh` cookies. The old refresh token
is marked `revoked = true`.

**Reuse detection**: if the presented refresh token was already
revoked, the server *also* revokes every other refresh token for the
same user and returns `401 unauthorized`. This forces a fresh login on
all sessions if a token leak is suspected.

**Errors**:

- `401 unauthorized` — missing, expired, or revoked token.

### `POST /api/auth/logout` (or `GET /api/auth/logout`)

Revoke the current refresh token and clear cookies. Both verbs are
accepted so plain `<a>` navigation links from the HUD work without
JavaScript. After clearing cookies the response is a `303 See Other`
redirect to `/`.

**Auth**: any session cookie (best-effort; idempotent).

**Response** `200 OK`:

```json
{ "ok": true }
```

### `GET /api/auth/me`

Return the current user record.

**Auth**: `gc_access` required.

**Response** `200 OK`:

```json
{
  "user": {
    "id": "01923ef0-...",
    "discord_id": "987654321098765432",
    "github_username": null,
    "email": "joachim.goehakue@epitech.eu",
    "email_verified": true,
    "avatar_url": "https://cdn.discordapp.com/avatars/...",
    "avatar_custom_url": null,
    "xp_total": 0,
    "level": 1,
    "global_rank": "Visitor",
    "bureau_role": null,
    "current_title": null,
    "streak_days": 0,
    "last_activity_at": null,
    "created_at": "2026-04-12T08:31:00Z"
  }
}
```

The shape mirrors `gamecloud_shared::models::UserRecord` exactly.

---

## QR attendance

### `POST /api/qr/generate`

Generate a single-use QR token for an event.

**Auth**: `gc_access` + `Authority::can(GenerateQrToken)`. That is
satisfied by any executive Bureau role and by `EventManager` /
`AssistantEventManager`.

**Request**:

```json
{
  "event_name": "Game Jam Mars 2026",
  "event_type": "GameJam",
  "xp_value": 100,
  "ttl_seconds": 7200
}
```

`event_type` ∈ `Session` | `OfficeHours` | `StandUp` | `GameJam` | `Special`.
`ttl_seconds` is optional (defaults to `JWT_QR_TTL`, capped at 86 400).

**Response** `200 OK`:

```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOi...",
  "qr_svg": "<svg ... </svg>",
  "expires_at": "2026-05-03T19:30:00Z"
}
```

The `token` is the JWT payload. The same string is encoded in the
SVG; the front-end displays the SVG full-screen on a projector.

**Errors**:

- `403 forbidden` — actor lacks `GenerateQrToken`.
- `422 validation` — bad `event_type` or negative `xp_value`.

### `POST /api/qr/scan`

Submit a token to record attendance and grant XP.

**Auth**: `gc_access` + `email_verified = true`.

**Request**:

```json
{ "token": "eyJ0eXAiOiJKV1QiLCJhbGciOi..." }
```

**Response** `200 OK`:

```json
{
  "event_name": "Game Jam Mars 2026",
  "event_type": "GameJam",
  "xp_awarded": 100
}
```

**Side effects** (single transaction):

1. JWT signature and audience (`qr`) verified.
2. `qr_tokens` row locked with `SELECT … FOR UPDATE`.
3. Reject if already used or expired.
4. `is_used` set to `true`.
5. `attendance` row inserted.
6. `xp_logs` row inserted with `source = QR`.
7. `users.xp_total` and `last_activity_at` updated.

**Errors**:

- `403 email_not_verified` — user has not completed OTP.
- `410 invalid_qr_token` — unknown, expired, or already-consumed
  token.

---

## Webhooks (machine-to-machine)

These endpoints are hit by external systems (GitHub) or by the bot
acting on behalf of Discord. None of them use cookies.

### `POST /api/webhooks/github`

GitHub webhook ingress.

**Authentication**: `X-Hub-Signature-256: sha256=<hex>` over the raw
body, HMAC-SHA256 with `GITHUB_WEBHOOK_SECRET`. The handler reads the
raw bytes, computes the MAC, and rejects with `401 unauthorized` on
any mismatch, missing header, malformed prefix, or wrong length.

**Headers**:

| Header                  | Purpose                                  |
| ----------------------- | ---------------------------------------- |
| `X-GitHub-Event`        | `push`, `pull_request`, `pull_request_review`, `issues`, `ping` |
| `X-Hub-Signature-256`   | HMAC-SHA256 over the raw body            |

**Behavior** (per event):

| Event                 | XP source | Action                                                                                                                  |
| --------------------- | --------- | ----------------------------------------------------------------------------------------------------------------------- |
| `ping`                | —         | Returns `200 pong`.                                                                                                     |
| `push`                | `GitHub`  | Awards `XP_GITHUB_COMMIT × n_commits`, capped at the daily limit of `XP_GITHUB_COMMIT_DAILY_CAP = 50`. Multipliers applied. |
| `pull_request`        | `GitHub`  | When `action = closed && merged = true`, awards `XP_GITHUB_PR_MERGED = 30`.                                              |
| `pull_request_review` | `GitHub`  | When `action = submitted`, awards `XP_GITHUB_REVIEW = 15`.                                                              |
| `issues`              | `GitHub`  | When `action = closed`, awards `XP_GITHUB_ISSUE_RESOLVED = 20` to the closing user.                                      |
| (any other)           | —         | Ignored.                                                                                                                |

The user is resolved by `users.github_username`. Unknown logins are
silently ignored (`200 OK`, no XP).

**Multipliers** (applied after the daily cap clamp):

```
final_xp = round(base * streak_multiplier * multi_track_bonus)
```

with the values from `gamecloud_shared::xp`:

| streak_days | multiplier |
| ----------- | ---------- |
| 0..6        | 1.00       |
| 7..13       | 1.25       |
| 14..29      | 1.50       |
| 30+         | 2.00       |

| active tracks | bonus |
| ------------- | ----- |
| 0..1          | 1.00  |
| 2             | 1.10  |
| 3             | 1.20  |
| 4+            | 1.30  |

Each XP grant writes:

- one row to `xp_logs`,
- a balance update on `users.xp_total` (+ rank bump if a threshold is
  crossed and the user is verified),
- a balance update on the matching `track_memberships.track_xp` when
  the source is track-scoped.

**Response**: `200 OK` body `ok` on accepted events; `200 OK` body
`pong` on `ping`. `401 unauthorized` for signature failures.

### `POST /api/sync/draftbot`

Posted by the GameCloud Discord bot when DraftBot announces a level
up in the configured channel. The bot parses the embed and forwards a
typed event here.

**Authentication**: `X-API-Key` matched in constant time against
`DRAFTBOT_API_KEY`.

**Request**:

```json
{
  "discord_id": "987654321098765432",
  "new_level": 14
}
```

**Response** `200 OK`:

- `ok` — XP granted.
- `ignored` — the Discord ID is unknown to GameCloud (the user has
  not registered yet).

**XP formula**: `bonus = max(new_level * 5, 0)`, then multipliers, then
recorded with `source = Discord`.

**Errors**:

- `401 unauthorized` — wrong or missing API key.

---

## Future endpoints (not yet implemented)

The following routes are reserved by the design and will land in
phases 3–4. Listing them here so the URL space is documented:

| Path                                | Method | Purpose                                  |
| ----------------------------------- | ------ | ---------------------------------------- |
| `/api/users/{id}`                   | GET    | Public profile                           |
| `/api/users/{id}/timeline`          | GET    | XP / activity feed                       |
| `/api/tracks/{track}/leaderboard`   | GET    | Per-track leaderboard                    |
| `/api/tracks/{track}/members`       | GET    | Members + their TrackRole                |
| `/api/projects`                     | GET    | List released projects (Hall of Fame)    |
| `/api/projects/{id}`                | GET    | Project detail                           |
| `/api/projects`                     | POST   | Create draft project                     |
| `/api/projects/{id}/submit`         | POST   | Draft → InReview                         |
| `/api/projects/{id}/validate`       | POST   | Per-track Approve/Reject                 |
| `/api/projects/{id}/publish`        | POST   | Approved → Released                      |
| `/api/quests`                       | GET    | Active quests                            |
| `/api/admin/bureau`                 | POST   | Assign / revoke Bureau roles             |
| `/api/admin/xp/grant`               | POST   | Manual XP grant (audited)                |
| `/api/admin/audit`                  | GET    | Audit log viewer                         |

All of them follow the conventions above (cookie auth,
`Authority::can` permission gate, JSON error envelope).

---

# Members

All of these require a session cookie.

### `GET /api/users/me`

The caller's profile: display name, avatar, XP, level and level
progress, rank identifier plus its title and colour, bureau role,
streak, and position on the all-time board.

### `PATCH /api/users/me`

```json
{ "current_title": "Barde Numérique", "github_username": "ada", "avatar_custom_url": "https://…" }
```

Every field is optional; omitted fields are left alone. Deliberately
narrow — XP, rank, level, streak and bureau role are platform-owned and
cannot be set here.

- `422 validation` — title outside 1–48 characters, avatar not https,
  GitHub login malformed.
- `409 conflict` — that GitHub login is already linked to another member.

### `GET /api/users/me/xp?limit=30`

Recent ledger lines: signed amount, source, track, description,
timestamp. `limit` is clamped to 200.

### `GET /api/users/me/tracks`

The caller's track memberships with role, specialization and track XP.

### `POST /api/users/me/tracks`

```json
{ "track": "Engineering", "specialization": "Gameplay" }
```

Join a track. **This is the step that completes onboarding**: the first
join promotes a `Visitor` to `Initiate` and puts them on the XP ladder.
`specialization` is optional but must be one of the canonical options
for that track.

- `403 email_not_verified` — verify the Epitech address first.
- `409 already_in_track`
- `422 unknown_track` / `unknown_specialization`

### `DELETE /api/users/me/tracks/{track}`

Leave a track.

### `GET /api/users/me/badges` · `GET /api/users/me/attendance` · `GET /api/users/me/quests`

The caller's badges, attendance history, and open quests.

### `GET /api/users/me/sheet`

The whole character sheet in one payload: profile, tracks, badges,
recent XP, attendance, completed quests. This is what a member can point
at from a portfolio.

### `GET /api/users/catalogue/tracks` · `GET /api/users/catalogue/badges`

The 8 tracks with their specializations, and the full badge catalogue.
Static; no authentication needed beyond a session.

### `GET /api/users/leaderboard?scope=season&track=&season=&limit=20`

| `scope` | Meaning |
|---|---|
| `season` *(default)* | XP earned inside the open season |
| `all` | all-time `users.xp_total` |
| `track` | `track_xp` inside the track named by `track` |

With `scope=season` and no season open, the response falls back to
`scope: "all"` rather than returning an empty board.

### `GET /api/users/{id}`

Another member's public profile.

---

# Quests

### `GET /api/quests`

Open quests with the caller's progress and completion flag. `Hidden`
quests only appear once the caller has made progress on them.

### `GET /api/quests/completed`

The caller's finished quests.

### `POST /api/quests`

Requires `GrantManualXp` (executive Bureau).

```json
{
  "title": "Trois revues cette semaine",
  "description": "Relis le build de quelqu'un d'autre.",
  "xp_reward": 40,
  "quest_type": "Weekly",
  "condition_type": "Review",
  "condition_value": 3,
  "track": null,
  "starts_at": "2026-09-14T00:00:00Z",
  "ends_at": "2026-09-21T00:00:00Z"
}
```

`quest_type` ∈ `Weekly | Special | Hidden`.
`condition_type` ∈ `Push | Attend | Submit | Review`.

Progress advances by itself from the XP events already flowing in; there
is no endpoint to increment a counter.

---

# Projects

### `GET /api/projects?track=&internal=false`

Released and archived projects by default. `internal=true` is honoured
only for a caller who holds the matching track role (or can access the
admin panel when no track filter is given).

### `POST /api/projects`

Requires `CreateProject` (rank ≥ `JuniorDev`). Creates a `Draft` and
credits the author as a contributor under the primary track.

```json
{
  "name": "Aevaryn",
  "short_description": "Un RPG narratif",
  "primary_track": "Narrative",
  "epitech_level": "Tek2",
  "block_number": 3,
  "github_repo_url": "https://github.com/…"
}
```

### `GET /api/projects/{id}`

Full detail: description, links, gallery, contributors, and the
per-track verdict table.

### `POST /api/projects/{id}/contributors`

```json
{ "user_id": "…", "track": "Audio", "role_in_project": "Composer" }
```

### `POST /api/projects/{id}/submit`

Requires `SubmitProjectForReview`. Moves `Draft | PartialOK | Rejected`
→ `InReview` and opens one validation row per concerned track — the
primary track plus every track a contributor is credited under. Returns
the tracks that now owe a verdict.

### `POST /api/projects/{id}/review`

Requires `ReviewProjectForTrack(track)` — that is, `Reviewer` or above
**in the track being judged**.

```json
{ "track": "Audio", "verdict": "Rejected", "feedback": "Le mix sature sur les explosions." }
```

`verdict` ∈ `Approved | Rejected | NotApplicable`. **A rejection must
carry feedback** (422 otherwise). The project status is re-derived from
all verdicts: any rejection → `Rejected`; any pending → still
`InReview`; at least one approval → `Approved`; nothing but
`NotApplicable` → `PartialOK`.

A substantive verdict earns the reviewer peer-review XP in that track.
`NotApplicable` pays nothing — declaring a track irrelevant is not
review work.

### `POST /api/projects/{id}/release`

Requires `PublishProjectAsReleased(primary_track)` — the track Lead.
Moves `Approved` → `Released`, derives rarity from the number of
approving tracks, pays every contributor and the primary Lead, and
writes the Hall of Fame entry. All in one transaction.

---

# Resources

### `GET /api/resources?track=&pending=false`

Validated entries, best-voted first. `pending=true` is honoured only for
a caller who can validate.

### `POST /api/resources`

```json
{ "title": "…", "url": "https://…", "resource_type": "Tutorial", "tracks": ["VisualArt"], "level": "Junior" }
```

`resource_type` ∈ `Tutorial | Tool | Asset | Doc | Video`.
`level` ∈ `Initiate | Junior | Senior | Expert`. The URL must be http(s).

### `POST /api/resources/{id}/validate`

Requires `ValidateResource` (Archiviste or any track Lead). Pays the
submitter. Returns `{"newly_validated": false}` on a repeat — the XP is
tied to the transition, not the click.

### `POST /api/resources/{id}/vote` · `DELETE /api/resources/{id}/vote`

One vote per member. `409 already_voted` on a repeat.

---

# Seasons

### `GET /api/seasons/current`

The open season, or `null` between seasons.

### `GET /api/seasons`

Every season, newest first.

---

# Admin

All require a Bureau permission; all write an `audit_logs` entry.

### `GET /api/admin/audit?limit=100`

Requires `ViewAuditLogs` (executive). The trail, newest first.

### `POST /api/admin/xp`

Requires `GrantManualXp`.

```json
{ "user_id": "…", "amount": 50, "reason": "Aide au montage du stand", "track": null }
```

`amount` may be negative to revoke. **Multipliers are not applied** —
when a Bureau member types 50, the recipient gets 50. A reason is
mandatory.

### `POST /api/admin/bureau-role`

Requires `AssignBureauRole` (President / Vice-President). `role: null`
clears it.

### `POST /api/admin/track-role`

```json
{ "user_id": "…", "track": "Engineering", "role": "Lead" }
```

`Lead` requires `AppointTrackLead` (executive); anything else requires
`AppointTrackCoLead` (the track's own Lead).

### `POST /api/admin/badge`

```json
{ "user_id": "…", "badge_type": "FoundingMember" }
```

Only the four Bureau-owned badges can be granted this way
(`FoundingMember`, `Alumni`, `ExternalMentor`, `BugHunter`). Attempting
to hand-award an engine-owned badge is a `422` — the next evaluation
would contradict it.

### `GET /api/admin/seasons` · `POST /api/admin/seasons`

List and open seasons. An overlapping window is a `409` — at most one
season may be open at a time.

---

# QR attendance (additions)

### `POST /api/qr/generate` — new fields

`max_scans` (optional) caps how many members may claim the token.
`None` means "everyone in the room before it expires", which is the
right default for a code on a projector. The response now also carries
`token_id`, for the attendance sheet.

### `POST /api/qr/scan` — changed semantics

**A token is no longer consumed by the first scanner.** It stays valid
until it expires or hits `max_scans`, and each member may claim it once.
The response carries `xp_awarded` (after the member's multipliers) and
`scan_count`.

- `409 qr_already_claimed` — you already scanned this code.
- `410 qr_capacity_reached` — the code is full.
- `410 invalid_qr_token` — unknown, expired, or a bad signature.

### `GET /api/qr/{id}/sheet`

Requires `GenerateQrToken`. Who has claimed a given token.

### `GET /api/qr/history`

The caller's own attendance history.
