-- Remise à zéro de la progression de tout le monde.
--
--   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/reset-progression.sql
--
-- Ce qui est remis à zéro : XP, titre global, série, quêtes en cours et
-- accomplies, badges obtenus automatiquement, choix de tracks et titres
-- de track, postes du Bureau.
--
-- Ce qui est reporté : l'XP Kumo de chaque membre, dans son XP globale —
-- donc son titre de membre — et nulle part ailleurs (ni track, ni poste).
-- Les inscrits la reçoivent ici ; les autres à leur première connexion.
-- RELEVER LES VALEURS KUMO (/top) JUSTE AVANT DE LANCER : elles bougent.
-- La plateforme doit avoir démarré une fois (migration 0018).
--
-- Ce qui est gardé : le poste du Président (franckalain07) et du
-- Vice-président (fred040), tous deux inscrits dans les 8 tracks mais
-- repartant comme Observateur à 0 XP ; les projets, présences, événements,
-- ressources, badges décernés à la main et le journal d'audit.
--
-- Discord suit tout seul : à la passe de synchronisation suivante, le bot
-- retire les rangs, tracks et postes qui ne correspondent plus.
--
-- Tout se fait dans une transaction : une erreur n'applique rien.

BEGIN;

-- Les membres du Bureau qui ne se sont jamais connectés ont leur compte
-- créé d'avance : leur poste et leur XP existent déjà, et leur première
-- connexion Discord retrouvera la ligne (même discord_id) au lieu d'en
-- créer une autre. Le bot complète les noms d'affichage.
INSERT INTO users (discord_id, discord_username) VALUES
    ('1224012500735889408', 'fred040'),          -- Fred, Vice-président
    ('1445689403249918013', 'inari351_26072'),   -- Kim, Secrétaire
    ('859179508988903464',  'dragon710538'),     -- Erwan
    ('1298589355371139144', 'y4n1s_79131'),      -- Yan
    ('1290263652158672898', 'yange4legend'),
    ('886739537114574868',  'crimad')
ON CONFLICT (discord_id) DO NOTHING;

DELETE FROM xp_logs;
DELETE FROM quest_progress;
DELETE FROM quest_completions;
DELETE FROM special_badges WHERE awarded_by IS NULL;

DO $$
BEGIN
    IF to_regclass('public.draftbot_pending_levels') IS NOT NULL THEN
        DELETE FROM draftbot_pending_levels;
    END IF;
END $$;

UPDATE users
   SET xp_total        = 0,
       level           = 1,
       streak_days     = 0,
       longest_streak  = 0,
       last_streak_day = NULL,
       current_title   = NULL,
       bureau_role     = CASE discord_id
                             WHEN '865973472223428608'  THEN 'President'
                             WHEN '1224012500735889408' THEN 'VicePresident'
                             WHEN '1445689403249918013' THEN 'Secretary'
                         END,
       -- Sans track, un membre vérifié repasse par le choix des tracks
       -- (Visitor). Président et Vice-président gardent les leurs.
       global_rank     = CASE
                             WHEN NOT email_verified THEN 'Pending'
                             WHEN discord_id IN ('865973472223428608', '1224012500735889408')
                                 THEN 'Initiate'
                             ELSE 'Visitor'
                         END;

DELETE FROM track_memberships
 WHERE user_id NOT IN (
     SELECT id FROM users
      WHERE discord_id IN ('865973472223428608', '1224012500735889408')
 );

INSERT INTO track_memberships (user_id, track, track_role, track_xp, last_active_at)
SELECT u.id, t.track, 'Observer', 0, NOW()
  FROM users u
 CROSS JOIN (VALUES ('Engineering'), ('GameDesign'), ('Narrative'), ('VisualArt'),
                    ('Audio'), ('Production'), ('QA'), ('Marketing')) AS t(track)
 WHERE u.discord_id IN ('865973472223428608', '1224012500735889408')
ON CONFLICT (user_id, track) DO UPDATE
   SET track_role = 'Observer',
       track_xp   = 0,
       left_at    = NULL;

-- ---------------------------------------------------------------------
-- Report de l'XP Kumo vers le titre de membre
-- ---------------------------------------------------------------------

DO $$
BEGIN
    IF to_regclass('public.legacy_xp') IS NULL THEN
        RAISE EXCEPTION 'Table legacy_xp absente : démarrez la plateforme une fois pour appliquer les migrations.';
    END IF;
END $$;

DELETE FROM legacy_xp;

-- Relevé Kumo « Top activité » du 2026-09-14 21:54 UTC. Seuls ces membres
-- reçoivent de l'XP ; tous les autres repartent à zéro.
INSERT INTO legacy_xp (discord_id, xp) VALUES
    ('1224012500735889408', 15344),  -- fred040
    ('865973472223428608',   7830),  -- franckalain07
    ('1290263652158672898',  6627),  -- yange4legend
    ('938513421610151966',   2431),  -- tha_b0y05
    ('1319735527171293288',  2012),  -- ancre_vocatif
    ('1120066975054434404',  1853),  -- pride_ad
    ('471477907005505537',   1742),  -- exodusky_neo
    ('859179508988903464',   1610),  -- dragon710538
    ('989955779907973130',   1571),  -- arthur_hara
    ('1286055256878481581',  1543);  -- libero_emargine

INSERT INTO xp_logs (user_id, amount, source, description, season_id)
SELECT u.id, l.xp, 'Discord', 'Report de l''XP Kumo',
       (SELECT id FROM seasons WHERE NOW() >= starts_at AND NOW() < ends_at
         ORDER BY starts_at DESC LIMIT 1)
  FROM legacy_xp l JOIN users u ON u.discord_id = l.discord_id;

UPDATE users u
   SET xp_total = l.xp,
       level    = GREATEST(1, floor((1 + sqrt(1 + 8 * l.xp / 100.0)) / 2)::int)
  FROM legacy_xp l
 WHERE l.discord_id = u.discord_id;

UPDATE legacy_xp SET credited_at = NOW()
 WHERE discord_id IN (SELECT discord_id FROM users);

-- Le titre suit l'XP pour ceux qui sont déjà sur l'échelle. Les autres
-- (Pending, Visitor) l'obtiennent en terminant leur inscription.
UPDATE users
   SET global_rank = CASE
           WHEN xp_total >= 25000 THEN 'Myth'
           WHEN xp_total >= 10000 THEN 'Legend'
           WHEN xp_total >= 5000  THEN 'Veteran'
           WHEN xp_total >= 2500  THEN 'Expert'
           WHEN xp_total >= 1000  THEN 'SeniorDev'
           WHEN xp_total >= 400   THEN 'JuniorDev'
           WHEN xp_total >= 150   THEN 'Apprentice'
           ELSE 'Initiate'
       END
 WHERE global_rank = 'Initiate';

SELECT discord_username, bureau_role, global_rank, xp_total,
       (SELECT count(*) FROM track_memberships m WHERE m.user_id = u.id AND m.left_at IS NULL) AS tracks
  FROM users u
 ORDER BY bureau_role NULLS LAST, discord_username;

COMMIT;
