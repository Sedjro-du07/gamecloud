//! Internal game jams — teams, entries, votes and results.
//!
//! The phases and the way winners are decided live in
//! `gamecloud_shared::jams`; this module checks every rule against the
//! database: one team per person per jam, the team size, a phase that
//! allows the action, no vote for one's own team, results published once.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use gamecloud_shared::{
    jams::{winner, Phase, Vote},
    roles::SpecialBadge,
    xp::XpSource,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    db::queries::{audit, xp},
    error::{WebError, WebResult},
    services::notifications::{self, Announcement},
};

/// A jam.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Jam {
    /// Row id.
    pub id: Uuid,
    /// Title.
    pub title: String,
    /// What it is about.
    pub description: Option<String>,
    /// Rules.
    pub rules: Option<String>,
    /// Theme, revealed when it starts.
    pub theme: String,
    /// Start: the theme is revealed.
    pub starts_at: DateTime<Utc>,
    /// Submissions close.
    pub ends_at: DateTime<Utc>,
    /// Voting closes.
    pub voting_ends_at: DateTime<Utc>,
    /// Largest team.
    pub max_team_size: i16,
    /// XP for everybody who handed in a game.
    pub participation_xp: i32,
    /// When the results were published.
    pub results_published_at: Option<DateTime<Utc>>,
    /// Teams registered.
    pub teams: i64,
    /// Games handed in.
    pub entries: i64,
}

impl Jam {
    /// Where the jam stands at `now`.
    #[must_use]
    pub fn phase(&self, now: DateTime<Utc>) -> Phase {
        Phase::at(
            now.timestamp(),
            self.starts_at.timestamp(),
            self.ends_at.timestamp(),
            self.voting_ends_at.timestamp(),
            self.results_published_at.is_some(),
        )
    }
}

const SELECT_JAM: &str = r#"
    SELECT j.id, j.title, j.description, j.rules, j.theme, j.starts_at, j.ends_at,
           j.voting_ends_at, j.max_team_size, j.participation_xp, j.results_published_at,
           (SELECT COUNT(*) FROM jam_teams t WHERE t.jam_id = j.id) AS teams,
           (SELECT COUNT(*) FROM jam_entries e WHERE e.jam_id = j.id) AS entries
      FROM game_jams j
"#;

/// Every jam, the latest first.
///
/// # Errors
/// Propagates database errors.
pub async fn list(pool: &PgPool) -> WebResult<Vec<Jam>> {
    Ok(sqlx::query_as::<_, Jam>(&format!("{SELECT_JAM} ORDER BY j.starts_at DESC LIMIT 100"))
        .fetch_all(pool)
        .await?)
}

/// One jam.
///
/// # Errors
/// `NotFound` when it does not exist.
pub async fn find(pool: &PgPool, id: Uuid) -> WebResult<Jam> {
    sqlx::query_as::<_, Jam>(&format!("{SELECT_JAM} WHERE j.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// A reward category.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Category {
    /// Row id.
    pub id: Uuid,
    /// "Meilleur jeu".
    pub name: String,
    /// What the winners receive besides XP.
    pub reward: Option<String>,
    /// XP for each member of the winning team.
    pub xp_reward: i32,
}

/// A jam's categories, in the Bureau's order.
///
/// # Errors
/// Propagates database errors.
pub async fn categories(pool: &PgPool, jam_id: Uuid) -> WebResult<Vec<Category>> {
    Ok(sqlx::query_as::<_, Category>(
        "SELECT id, name, reward, xp_reward FROM jam_categories WHERE jam_id = $1 ORDER BY position",
    )
    .bind(jam_id)
    .fetch_all(pool)
    .await?)
}

/// A jam about to be opened.
#[derive(Debug, Clone)]
pub struct NewJam {
    /// Title.
    pub title: String,
    /// What it is about.
    pub description: Option<String>,
    /// Rules.
    pub rules: Option<String>,
    /// Theme, hidden until the start.
    pub theme: String,
    /// Start.
    pub starts_at: DateTime<Utc>,
    /// Submissions close.
    pub ends_at: DateTime<Utc>,
    /// Voting closes.
    pub voting_ends_at: DateTime<Utc>,
    /// Largest team.
    pub max_team_size: i16,
    /// XP for everybody who hands in a game.
    pub participation_xp: i32,
    /// Categories: name, reward, XP.
    pub categories: Vec<(String, Option<String>, i32)>,
}

fn invalid<T>(message: &str) -> WebResult<T> {
    Err(WebError::Validation(message.to_string()))
}

/// Check a jam before it is opened.
///
/// # Errors
/// `Validation` describing the first problem, in French.
pub fn validate(new: &NewJam, now: DateTime<Utc>) -> WebResult<()> {
    let title = new.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return invalid("donne un titre à la jam (120 caractères maximum)");
    }
    let theme = new.theme.trim();
    if theme.is_empty() || theme.chars().count() > 200 {
        return invalid("indique le thème (200 caractères maximum) : il restera caché jusqu'au début");
    }
    if new.starts_at <= now {
        return invalid("le début doit être dans le futur");
    }
    if new.ends_at <= new.starts_at {
        return invalid("la fin des rendus doit venir après le début");
    }
    if new.voting_ends_at <= new.ends_at {
        return invalid("la fin du vote doit venir après la fin des rendus");
    }
    if !(1..=10).contains(&new.max_team_size) {
        return invalid("une équipe compte de 1 à 10 personnes");
    }
    if !(0..=1000).contains(&new.participation_xp) {
        return invalid("l'XP de participation va de 0 à 1000");
    }
    if new.categories.is_empty() {
        return invalid("ajoute au moins une catégorie de récompense");
    }
    let mut seen = std::collections::HashSet::new();
    for (name, _, xp) in &new.categories {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return invalid("chaque catégorie a un nom (60 caractères maximum)");
        }
        if !seen.insert(name.to_lowercase()) {
            return invalid("deux catégories portent le même nom");
        }
        if !(0..=2000).contains(xp) {
            return invalid("l'XP d'une catégorie va de 0 à 2000");
        }
    }
    Ok(())
}

/// Open a jam and announce it.
///
/// # Errors
/// `Validation` for an invalid jam.
pub async fn create(pool: &PgPool, channels: DiscordChannels, new: &NewJam, by: Uuid) -> WebResult<Uuid> {
    validate(new, Utc::now())?;
    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO game_jams (title, description, rules, theme, starts_at, ends_at, voting_ends_at,
                               max_team_size, participation_xp, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(new.title.trim())
    .bind(new.description.as_deref().map(str::trim).filter(|d| !d.is_empty()))
    .bind(new.rules.as_deref().map(str::trim).filter(|r| !r.is_empty()))
    .bind(new.theme.trim())
    .bind(new.starts_at)
    .bind(new.ends_at)
    .bind(new.voting_ends_at)
    .bind(new.max_team_size)
    .bind(new.participation_xp)
    .bind(by)
    .fetch_one(&mut *tx)
    .await?;
    for (position, (name, reward, xp_reward)) in new.categories.iter().enumerate() {
        sqlx::query(
            "INSERT INTO jam_categories (jam_id, name, reward, xp_reward, position) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(name.trim())
        .bind(reward.as_deref().map(str::trim).filter(|r| !r.is_empty()))
        .bind(xp_reward)
        .bind(i16::try_from(position).unwrap_or(i16::MAX))
        .execute(&mut *tx)
        .await?;
    }
    audit::record_in_tx(&mut tx, Some(by), "jam.created", Some("game_jam"), Some(id), serde_json::json!({ "title": new.title.trim() })).await?;
    notifications::enqueue(
        &mut tx,
        channels,
        &Announcement::jam(
            "🎮 Nouvelle game jam",
            format!(
                "**{}**\nDébut le {} : le thème sera dévoilé à ce moment-là. Inscris ton équipe sur la plateforme, page Game jams.",
                new.title.trim(),
                local_stamp(new.starts_at)
            ),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// A moment as the page shows it. Like the calendar, the time the
/// organiser typed is the time everybody reads: no zone conversion.
#[must_use]
pub fn local_stamp(at: DateTime<Utc>) -> String {
    at.format("%d/%m/%Y à %H:%M").to_string()
}

/// Delete a jam that has not started. Returns the stored files to remove.
///
/// # Errors
/// `Validation` once it has started; `NotFound` when it does not exist.
pub async fn delete(pool: &PgPool, id: Uuid, by: Uuid) -> WebResult<Vec<String>> {
    let jam = find(pool, id).await?;
    if jam.phase(Utc::now()) != Phase::Registration {
        return invalid("une jam commencée ne se supprime plus");
    }
    let mut tx = pool.begin().await?;
    let files: Vec<String> = sqlx::query_scalar(
        "SELECT storage_name FROM jam_entries WHERE jam_id = $1 AND storage_name IS NOT NULL",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM game_jams WHERE id = $1").bind(id).execute(&mut *tx).await?;
    audit::record_in_tx(&mut tx, Some(by), "jam.deleted", Some("game_jam"), Some(id), serde_json::json!({ "title": jam.title })).await?;
    tx.commit().await?;
    Ok(files)
}

/// A team, its members and its game.
#[derive(Debug, Clone)]
pub struct Team {
    /// Row id.
    pub id: Uuid,
    /// Name.
    pub name: String,
    /// Members' Discord usernames.
    pub members: Vec<String>,
    /// Members' accounts.
    pub member_ids: Vec<Uuid>,
    /// The game handed in, if any.
    pub entry: Option<Entry>,
}

/// A game handed in.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Entry {
    /// Title.
    pub title: String,
    /// What it is.
    pub description: Option<String>,
    /// A link to play or download it.
    pub url: Option<String>,
    /// The file, if one was uploaded.
    pub filename: Option<String>,
    /// Its size.
    pub size_bytes: Option<i64>,
    /// Name it is stored under.
    pub storage_name: Option<String>,
    /// When it was (last) handed in.
    pub submitted_at: DateTime<Utc>,
}

/// A team's game, as read back with its team.
#[derive(sqlx::FromRow)]
struct EntryRow {
    team_id: Uuid,
    #[sqlx(flatten)]
    entry: Entry,
}

/// A jam's teams, those that handed in first first, then the others.
///
/// # Errors
/// Propagates database errors.
pub async fn teams(pool: &PgPool, jam_id: Uuid) -> WebResult<Vec<Team>> {
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM jam_teams WHERE jam_id = $1 ORDER BY created_at")
            .bind(jam_id)
            .fetch_all(pool)
            .await?;
    let members: Vec<(Uuid, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT m.team_id, u.id, u.discord_username FROM jam_team_members m JOIN users u ON u.id = m.user_id \
          WHERE m.jam_id = $1 ORDER BY m.joined_at",
    )
    .bind(jam_id)
    .fetch_all(pool)
    .await?;
    let entries: Vec<EntryRow> = sqlx::query_as(
        "SELECT team_id, title, description, url, filename, size_bytes, storage_name, submitted_at \
           FROM jam_entries WHERE jam_id = $1",
    )
    .bind(jam_id)
    .fetch_all(pool)
    .await?;
    let mut entries: HashMap<Uuid, Entry> = entries.into_iter().map(|r| (r.team_id, r.entry)).collect();
    let mut teams: Vec<Team> = rows
        .into_iter()
        .map(|(id, name)| {
            let mine: Vec<&(Uuid, Uuid, Option<String>)> = members.iter().filter(|m| m.0 == id).collect();
            Team {
                id,
                name,
                members: mine.iter().map(|m| m.2.clone().unwrap_or_default()).collect(),
                member_ids: mine.iter().map(|m| m.1).collect(),
                entry: entries.remove(&id),
            }
        })
        .collect();
    teams.sort_by_key(|t| t.entry.as_ref().map_or((1, None), |e| (0, Some(e.submitted_at))));
    Ok(teams)
}

async fn require_member(pool: &PgPool, user_id: Uuid) -> WebResult<()> {
    if crate::db::queries::users::is_member(pool, user_id).await? {
        Ok(())
    } else {
        invalid("les game jams sont réservées aux membres de l'association")
    }
}

/// The team a member is in for a jam, if any.
///
/// # Errors
/// Propagates database errors.
pub async fn team_of(pool: &PgPool, jam_id: Uuid, user_id: Uuid) -> WebResult<Option<Uuid>> {
    Ok(sqlx::query_scalar("SELECT team_id FROM jam_team_members WHERE jam_id = $1 AND user_id = $2")
        .bind(jam_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?)
}

/// Register a team, with its creator as first member.
///
/// # Errors
/// `Validation` when teams are closed, the name is taken or the member is
/// already in a team.
pub async fn create_team(pool: &PgPool, jam_id: Uuid, name: &str, user_id: Uuid) -> WebResult<Uuid> {
    require_member(pool, user_id).await?;
    let jam = find(pool, jam_id).await?;
    if !jam.phase(Utc::now()).teams_open() {
        return invalid("les inscriptions sont closes pour cette jam");
    }
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return invalid("donne un nom à ton équipe (60 caractères maximum)");
    }
    if team_of(pool, jam_id, user_id).await?.is_some() {
        return invalid("tu es déjà dans une équipe pour cette jam");
    }
    let mut tx = pool.begin().await?;
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO jam_teams (jam_id, name, created_by) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING RETURNING id",
    )
    .bind(jam_id)
    .bind(name)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(id) = id else {
        return invalid("une équipe porte déjà ce nom dans cette jam");
    };
    sqlx::query("INSERT INTO jam_team_members (team_id, jam_id, user_id) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(jam_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(id)
}

/// Add a member to one's team.
///
/// # Errors
/// `Validation` when teams are closed, the team is full, the person is not
/// a member or is already in a team; `Forbidden` when the caller is not in
/// the team.
pub async fn add_member(pool: &PgPool, jam_id: Uuid, by: Uuid, username: &str) -> WebResult<()> {
    let jam = find(pool, jam_id).await?;
    if !jam.phase(Utc::now()).teams_open() {
        return invalid("les inscriptions sont closes pour cette jam");
    }
    let Some(team) = team_of(pool, jam_id, by).await? else {
        return invalid("crée d'abord ton équipe");
    };
    let Some(target) = crate::db::queries::users::find_by_reference(pool, username).await? else {
        return invalid("personne n'a ce pseudo sur la plateforme");
    };
    if !crate::db::queries::users::is_member(pool, target).await? {
        return invalid("cette personne n'est pas membre de l'association");
    }
    let size: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jam_team_members WHERE team_id = $1")
        .bind(team)
        .fetch_one(pool)
        .await?;
    if size >= i64::from(jam.max_team_size) {
        return invalid("ton équipe est complète");
    }
    let added = sqlx::query(
        "INSERT INTO jam_team_members (team_id, jam_id, user_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(team)
    .bind(jam_id)
    .bind(target)
    .execute(pool)
    .await?;
    if added.rows_affected() == 0 {
        return invalid("cette personne est déjà dans une équipe pour cette jam");
    }
    Ok(())
}

/// Leave one's team. The last one out removes the team and its game;
/// returns the stored file to remove, if any.
///
/// # Errors
/// `Validation` once teams are closed or when not in a team.
pub async fn leave(pool: &PgPool, jam_id: Uuid, user_id: Uuid) -> WebResult<Option<String>> {
    let jam = find(pool, jam_id).await?;
    if !jam.phase(Utc::now()).teams_open() {
        return invalid("les équipes sont figées : la jam est terminée");
    }
    let Some(team) = team_of(pool, jam_id, user_id).await? else {
        return invalid("tu n'es dans aucune équipe pour cette jam");
    };
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM jam_team_members WHERE team_id = $1 AND user_id = $2")
        .bind(team)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jam_team_members WHERE team_id = $1")
        .bind(team)
        .fetch_one(&mut *tx)
        .await?;
    let mut file = None;
    if left == 0 {
        file = sqlx::query_scalar("SELECT storage_name FROM jam_entries WHERE team_id = $1")
            .bind(team)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
        sqlx::query("DELETE FROM jam_teams WHERE id = $1").bind(team).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(file)
}

/// A game about to be handed in.
#[derive(Debug, Clone, Default)]
pub struct NewEntry {
    /// Title.
    pub title: String,
    /// What it is.
    pub description: Option<String>,
    /// A link to play or download it.
    pub url: Option<String>,
    /// The uploaded file: original name, size, SHA-256, stored name.
    pub file: Option<(String, i64, String, String)>,
}

/// Hand in (or replace) the team's game. Returns the stored file it
/// replaced, if any.
///
/// # Errors
/// `Validation` outside the jam, without a title, or with neither a link
/// nor a file; when the caller is in no team.
pub async fn submit(pool: &PgPool, jam_id: Uuid, by: Uuid, new: &NewEntry) -> WebResult<Option<String>> {
    let jam = find(pool, jam_id).await?;
    if !jam.phase(Utc::now()).submissions_open() {
        return invalid("les rendus sont ouverts seulement pendant la jam");
    }
    let Some(team) = team_of(pool, jam_id, by).await? else {
        return invalid("rejoins ou crée une équipe pour rendre un jeu");
    };
    let title = new.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return invalid("donne un titre à ton jeu (120 caractères maximum)");
    }
    let url = new.url.as_deref().map(str::trim).filter(|u| !u.is_empty());
    if url.is_some_and(|u| !(u.starts_with("https://") || u.starts_with("http://"))) {
        return invalid("le lien doit commencer par https://");
    }
    let previous: Option<Option<String>> = sqlx::query_scalar("SELECT storage_name FROM jam_entries WHERE team_id = $1")
        .bind(team)
        .fetch_optional(pool)
        .await?;
    // A new file replaces the old one; without one, the old file stays.
    let keep_file = new.file.is_none();
    if url.is_none() && new.file.is_none() && previous.clone().flatten().is_none() {
        return invalid("ajoute un lien vers ton jeu ou son fichier");
    }
    let (filename, size, checksum, stored) = match &new.file {
        Some((f, s, c, st)) => (Some(f.as_str()), Some(*s), Some(c.as_str()), Some(st.as_str())),
        None => (None, None, None, None),
    };
    let description = new.description.as_deref().map(str::trim).filter(|d| !d.is_empty());
    if previous.is_some() {
        // A replacement. Without a new file the old one stays; checking the
        // "link or file" rule on an insert-then-update would refuse that.
        sqlx::query(
            r#"
            UPDATE jam_entries SET
                title = $2, description = $3, url = $4,
                filename     = CASE WHEN $9 THEN filename     ELSE $5 END,
                size_bytes   = CASE WHEN $9 THEN size_bytes   ELSE $6 END,
                checksum     = CASE WHEN $9 THEN checksum     ELSE $7 END,
                storage_name = CASE WHEN $9 THEN storage_name ELSE $8 END,
                submitted_at = NOW()
             WHERE team_id = $1
            "#,
        )
        .bind(team)
        .bind(title)
        .bind(description)
        .bind(url)
        .bind(filename)
        .bind(size)
        .bind(checksum)
        .bind(stored)
        .bind(keep_file)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO jam_entries (team_id, jam_id, title, description, url, filename, size_bytes, checksum, storage_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(team)
        .bind(jam_id)
        .bind(title)
        .bind(description)
        .bind(url)
        .bind(filename)
        .bind(size)
        .bind(checksum)
        .bind(stored)
        .execute(pool)
        .await?;
    }
    Ok(if keep_file { None } else { previous.flatten() })
}

/// The stored file of a team's game, with its name and size.
///
/// # Errors
/// `NotFound` when the team handed in no file.
pub async fn entry_file(pool: &PgPool, team_id: Uuid) -> WebResult<(String, String, i64)> {
    let row: Option<(Option<String>, Option<String>, Option<i64>)> =
        sqlx::query_as("SELECT storage_name, filename, size_bytes FROM jam_entries WHERE team_id = $1")
            .bind(team_id)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((Some(stored), Some(name), Some(size))) => Ok((stored, name, size)),
        _ => Err(WebError::NotFound),
    }
}

/// Give a game a score in a category.
///
/// # Errors
/// `Validation` outside the vote, for one's own team, a game not handed
/// in or a score outside 1–5.
pub async fn vote(
    pool: &PgPool,
    jam_id: Uuid,
    team_id: Uuid,
    category_id: Uuid,
    voter: Uuid,
    jury: bool,
    score: u8,
) -> WebResult<()> {
    require_member(pool, voter).await?;
    let jam = find(pool, jam_id).await?;
    if jam.phase(Utc::now()) != Phase::Voting {
        return invalid("le vote n'est pas ouvert");
    }
    if !(1..=5).contains(&score) {
        return invalid("une note va de 1 à 5");
    }
    if team_of(pool, jam_id, voter).await? == Some(team_id) {
        return invalid("tu ne votes pas pour ton propre jeu");
    }
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM jam_entries WHERE team_id = $1 AND jam_id = $3) \
            AND EXISTS (SELECT 1 FROM jam_categories WHERE id = $2 AND jam_id = $3)",
    )
    .bind(team_id)
    .bind(category_id)
    .bind(jam_id)
    .fetch_one(pool)
    .await?;
    if !valid {
        return invalid("ce jeu ou cette catégorie n'existe pas dans cette jam");
    }
    sqlx::query(
        r#"
        INSERT INTO jam_votes (category_id, team_id, voter_id, jam_id, score, jury)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (category_id, team_id, voter_id) DO UPDATE
           SET score = EXCLUDED.score, jury = EXCLUDED.jury, voted_at = NOW()
        "#,
    )
    .bind(category_id)
    .bind(team_id)
    .bind(voter)
    .bind(jam_id)
    .bind(i16::from(score))
    .bind(jury)
    .execute(pool)
    .await?;
    Ok(())
}

/// A voter's own scores: (team, category) → score.
///
/// # Errors
/// Propagates database errors.
pub async fn my_votes(pool: &PgPool, jam_id: Uuid, voter: Uuid) -> WebResult<HashMap<(Uuid, Uuid), u8>> {
    let rows: Vec<(Uuid, Uuid, i16)> = sqlx::query_as(
        "SELECT team_id, category_id, score FROM jam_votes WHERE jam_id = $1 AND voter_id = $2",
    )
    .bind(jam_id)
    .bind(voter)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(t, c, s)| ((t, c), u8::try_from(s).unwrap_or(0)))
        .collect())
}

/// Every vote of a jam, by (category, team).
///
/// # Errors
/// Propagates database errors.
pub async fn votes(pool: &PgPool, jam_id: Uuid) -> WebResult<HashMap<(Uuid, Uuid), Vec<Vote>>> {
    let rows: Vec<(Uuid, Uuid, bool, i16)> =
        sqlx::query_as("SELECT category_id, team_id, jury, score FROM jam_votes WHERE jam_id = $1")
            .bind(jam_id)
            .fetch_all(pool)
            .await?;
    let mut out: HashMap<(Uuid, Uuid), Vec<Vote>> = HashMap::new();
    for (c, t, jury, score) in rows {
        out.entry((c, t)).or_default().push(Vote { jury, score: u8::try_from(score).unwrap_or(0) });
    }
    Ok(out)
}

/// The winners, by category: (team, score).
///
/// # Errors
/// Propagates database errors.
pub async fn awards(pool: &PgPool, jam_id: Uuid) -> WebResult<HashMap<Uuid, (Uuid, f32)>> {
    let rows: Vec<(Uuid, Uuid, f32)> =
        sqlx::query_as("SELECT category_id, team_id, score FROM jam_awards WHERE jam_id = $1")
            .bind(jam_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(c, t, s)| (c, (t, s))).collect())
}

/// Publish the results: decide each category, pay XP and badges, announce.
/// Done once, after the vote closes, in one transaction.
///
/// # Errors
/// `Validation` before the vote closes or when already published.
pub async fn publish(pool: &PgPool, channels: DiscordChannels, jam_id: Uuid, by: Uuid) -> WebResult<()> {
    let jam = find(pool, jam_id).await?;
    match jam.phase(Utc::now()) {
        Phase::Deliberation => {}
        Phase::Results => return invalid("les résultats sont déjà publiés"),
        _ => return invalid("les résultats se publient une fois le vote clos"),
    }
    let cats = categories(pool, jam_id).await?;
    let teams = teams(pool, jam_id).await?;
    let entrants: Vec<&Team> = teams.iter().filter(|t| t.entry.is_some()).collect();
    let all_votes = votes(pool, jam_id).await?;
    let label = format!("Game jam : {}", jam.title);

    let mut tx = pool.begin().await?;
    let published = sqlx::query(
        "UPDATE game_jams SET results_published_at = NOW() WHERE id = $1 AND results_published_at IS NULL",
    )
    .bind(jam_id)
    .execute(&mut *tx)
    .await?;
    if published.rows_affected() == 0 {
        return invalid("les résultats sont déjà publiés");
    }

    // Everybody who handed in a game: participation XP and the Jammeur badge.
    for team in &entrants {
        for member in &team.member_ids {
            if jam.participation_xp > 0 {
                let mut grant = xp::XpGrant::new(*member, jam.participation_xp, XpSource::Project);
                grant.description = Some(&label);
                grant.apply_multipliers = false;
                xp::grant_in_tx(&mut tx, channels, &grant).await?;
            }
            award_badge(&mut tx, *member, SpecialBadge::GameJamParticipant, by).await?;
        }
    }

    // Each category: its winner, their XP and the winner's badge.
    let mut lines = Vec::new();
    for cat in &cats {
        let ranked: Vec<(Uuid, Vec<Vote>)> = entrants
            .iter()
            .map(|t| (t.id, all_votes.get(&(cat.id, t.id)).cloned().unwrap_or_default()))
            .collect();
        let Some(winning) = winner(&ranked) else {
            lines.push(format!("**{}** : aucun vote", cat.name));
            continue;
        };
        let votes_for = all_votes.get(&(cat.id, winning)).cloned().unwrap_or_default();
        let score = gamecloud_shared::jams::standing(winning, &votes_for).score.unwrap_or(0.0);
        #[allow(clippy::cast_possible_truncation)]
        let score32 = score as f32;
        sqlx::query("INSERT INTO jam_awards (category_id, jam_id, team_id, score) VALUES ($1, $2, $3, $4)")
            .bind(cat.id)
            .bind(jam_id)
            .bind(winning)
            .bind(score32)
            .execute(&mut *tx)
            .await?;
        let team = entrants.iter().find(|t| t.id == winning);
        if let Some(team) = team {
            for member in &team.member_ids {
                if cat.xp_reward > 0 {
                    let why = format!("{label}, {}", cat.name);
                    let mut grant = xp::XpGrant::new(*member, cat.xp_reward, XpSource::Project);
                    grant.description = Some(&why);
                    grant.apply_multipliers = false;
                    xp::grant_in_tx(&mut tx, channels, &grant).await?;
                }
                award_badge(&mut tx, *member, SpecialBadge::GameJamWinner, by).await?;
            }
            let reward = cat.reward.as_deref().map(|r| format!(" ({r})")).unwrap_or_default();
            lines.push(format!("**{}**{reward} : {} ({})", cat.name, team.name, team.members.join(", ")));
        }
    }

    audit::record_in_tx(&mut tx, Some(by), "jam.results_published", Some("game_jam"), Some(jam_id), serde_json::json!({ "title": jam.title })).await?;
    notifications::enqueue(
        &mut tx,
        channels,
        &Announcement::jam(
            "🏆 Résultats de la game jam",
            format!("**{}**\n{}\nBravo à toutes les équipes qui ont rendu un jeu !", jam.title, lines.join("\n")),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn award_badge(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    badge: SpecialBadge,
    by: Uuid,
) -> WebResult<()> {
    sqlx::query(
        "INSERT INTO special_badges (user_id, badge_type, awarded_by) VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, badge_type) DO NOTHING",
    )
    .bind(user_id)
    .bind(badge.as_str())
    .bind(by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn jam(now: DateTime<Utc>) -> NewJam {
        NewJam {
            title: "Jam d'hiver".into(),
            description: None,
            rules: None,
            theme: "Lumière".into(),
            starts_at: now + Duration::days(1),
            ends_at: now + Duration::days(3),
            voting_ends_at: now + Duration::days(5),
            max_team_size: 4,
            participation_xp: 100,
            categories: vec![("Meilleur jeu".into(), Some("Goodies".into()), 300)],
        }
    }

    #[test]
    fn a_well_formed_jam_passes() {
        let now = Utc::now();
        assert!(validate(&jam(now), now).is_ok());
    }

    #[test]
    fn dates_must_follow_each_other() {
        let now = Utc::now();
        let mut j = jam(now);
        j.ends_at = j.starts_at;
        assert!(validate(&j, now).is_err());
        let mut j = jam(now);
        j.voting_ends_at = j.ends_at;
        assert!(validate(&j, now).is_err());
        let mut j = jam(now);
        j.starts_at = now - Duration::hours(1);
        assert!(validate(&j, now).is_err());
    }

    #[test]
    fn categories_are_required_and_distinct() {
        let now = Utc::now();
        let mut j = jam(now);
        j.categories.clear();
        assert!(validate(&j, now).is_err());
        let mut j = jam(now);
        j.categories.push(("meilleur JEU".into(), None, 100));
        assert!(validate(&j, now).is_err());
    }

    #[test]
    fn times_show_as_typed() {
        let at = DateTime::parse_from_rfc3339("2026-12-01T09:00:00Z").unwrap().with_timezone(&Utc);
        assert_eq!(local_stamp(at), "01/12/2026 à 09:00");
    }
}
