#!/usr/bin/env bash
# Delete the Discord channels that duplicate another one.
#
# Run from the repository root. The message history of every channel below
# is already archived in docs/discord-channels-backup.md — Discord has no
# undelete, so do not run this if that file is missing.
#
# `🏆level` is deliberately absent: it is DRAFTBOT_CHANNEL_ID, the channel
# the bot reads DraftBot's level-ups from. Deleting it stops XP syncing.
set -euo pipefail

TOKEN=$(grep -oP '^DISCORD_TOKEN=\K.*' .env | tr -d '"')
[ -n "$TOKEN" ] || { echo "DISCORD_TOKEN introuvable dans .env" >&2; exit 1; }
[ -f docs/discord-channels-backup.md ] || { echo "Sauvegarde manquante, abandon." >&2; exit 1; }

# id|nom|ce qui le remplace
CHANNELS=(
  "1443521467252740200|🎭demande-roles|la plateforme — les tracks s'auto-assignent, le reste est un privilège"
  "1492575132621738157|⚙️commandes|📖gc-guide et les commandes slash"
  "1444592921402671185|📆agenda|le calendrier de la plateforme"
  "1444600460848795748|📚projet-en-cours|la page Projets et 🔍gc-a-valider (salon vide)"
  # Les salons suivants sont ceux *de la plateforme* qui disparaissent : le
  # salon d'origine existait avant, les membres le suivent déjà, et il a
  # une histoire — en créer un second à côté était l'erreur.
  "1549026114624823367|📣gc-annonces|📢annonces, où la plateforme publie désormais"
  "1549143916748607619|🏛gc-bureau|🏛bureau, où arrivent les réunions du Bureau"
  "1549026944484380754|🏛gc-journal|🏛bureau (salon vide, rien n'y publiait)"
)

for entry in "${CHANNELS[@]}"; do
  IFS='|' read -r id name why <<< "$entry"
  code=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE \
    -H "Authorization: Bot $TOKEN" \
    -H "X-Audit-Log-Reason: GameCloud OS migration: duplicate channel" \
    "https://discord.com/api/v10/channels/$id")
  printf '  %-22s HTTP %s   → %s\n' "$name" "$code" "$why"
  sleep 0.6
done
