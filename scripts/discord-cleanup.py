#!/usr/bin/env python3
"""Supprime les rôles Discord pré-existants du serveur Game Cloud.

À lancer depuis la racine du dépôt :

    python3 scripts/discord-cleanup.py --dry-run   # liste sans rien toucher
    python3 scripts/discord-cleanup.py             # supprime

Ce que le script NE touche pas, volontairement :

- les rôles d'intégration (Kumo, DraftBot, News Alerts Bot, Gamecloud Os) :
  Discord interdit leur suppression, ils sont liés aux bots ;
- les rôles qui conditionnent l'accès à des salons — `👑✨️Bureau✨️`
  (🏛bureau, 🏯Bureau), `The mute 🤐` (sanctions sur 💬général) et
  `Mister Martin Martin🤕😅` (📌 Informations, 👋bienvenue). Les supprimer
  casserait les permissions de ces salons ;
- tout ce que la plateforme a créé : les 10 rangs, les 16 postes du
  Bureau, les 8 tracks.

Une sauvegarde de qui portait quoi se trouve dans
`docs/discord-roles-backup.md`. Les attributions ne sont pas
récupérables autrement.
"""

from __future__ import annotations

import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

GUILD = "1443521466460278888"

# Rôles conservés bien qu'ils soient pré-existants : ils ouvrent des salons.
KEEP = {
    "👑✨️Bureau✨️",
    "The mute 🤐",
    "Mister Martin Martin🤕😅",
}

# Préfixes des rôles créés par la plateforme — jamais supprimés.
PLATFORM = [
    "⏳ L'Aspirant", "👁️ Observateur", "🌱 L'Initié", "📖 Apprenti Forgeron",
    "⚡ Compagnon", "🔥 Vétéran", "💎 Maître Artisan", "🌙 Ancien de la Forge",
    "🌟 Légende", "👑⚡ Mythe",
    "⚜️ Grand Archonte", "🛡️ Archonte", "📋 Scribe", "💰 Intendant",
    "🔧 Forgeron", "🌐 Héraut", "🎪 Maître des Arènes", "🎭 Écuyer",
    "📜 Gardien", "📂 Apprenti Chroniqueur", "📣 Éclaireur", "📸 Barde",
    "⚔️ Sentinelle", "🗡️ Garde", "🎯 Chasseur", "🤝 Ambassadeur",
    "⚙️ Engineering", "🎮 Game Design", "📖 Narrative", "🎨 Visual Art",
    "🎵 Audio", "📊 Production", "🐛 QA", "📣 Marketing",
]


def token() -> str:
    env = Path(".env")
    if not env.is_file():
        sys.exit("erreur : lancez le script depuis la racine du dépôt (.env introuvable)")
    match = re.search(r"^DISCORD_TOKEN=(.*)$", env.read_text(), re.M)
    if not match:
        sys.exit("erreur : DISCORD_TOKEN absent de .env")
    return match.group(1).strip().strip('"')


def api(tok: str, path: str, method: str = "GET"):
    req = urllib.request.Request(
        f"https://discord.com/api/v10{path}",
        method=method,
        headers={
            "Authorization": f"Bot {tok}",
            "User-Agent": "GameCloudOS cleanup",
            "X-Audit-Log-Reason": "GameCloud OS role cleanup",
        },
    )
    for _ in range(6):
        try:
            body = urllib.request.urlopen(req, timeout=25).read()
            return json.loads(body) if body else None
        except urllib.error.HTTPError as exc:
            if exc.code == 429:
                wait = float(json.loads(exc.read()).get("retry_after", 2)) + 0.5
                time.sleep(wait)
                continue
            raise RuntimeError(f"{method} {path} → {exc.code} "
                               f"{exc.read()[:160].decode(errors='replace')}") from exc
    raise RuntimeError("limite de débit Discord atteinte")


def is_platform(name: str) -> bool:
    return any(name == p or name.startswith(p) for p in PLATFORM)


def main() -> None:
    dry_run = "--dry-run" in sys.argv
    tok = token()

    roles = api(tok, f"/guilds/{GUILD}/roles")
    doomed = [
        r for r in sorted(roles, key=lambda r: -r["position"])
        if r["name"] != "@everyone"
        and not r.get("managed")
        and r["name"] not in KEEP
        and not is_platform(r["name"])
    ]

    if not doomed:
        print("Rien à supprimer — le serveur est déjà propre.")
        return

    print(f"{len(doomed)} rôle(s) à supprimer :")
    for r in doomed:
        print(f"   - {r['name']}")

    if dry_run:
        print("\n--dry-run : rien n'a été supprimé.")
        return

    backup = Path("docs/discord-roles-backup.md")
    print(f"\nSauvegarde des attributions : {backup}"
          if backup.is_file() else
          "\n⚠️  docs/discord-roles-backup.md introuvable — les attributions "
          "seront perdues sans trace.")
    answer = input("Confirmer la suppression ? (tapez OUI) ").strip()
    if answer != "OUI":
        print("Annulé.")
        return

    ok = failed = 0
    for r in doomed:
        try:
            api(tok, f"/guilds/{GUILD}/roles/{r['id']}", "DELETE")
            print(f"   ✓ {r['name']}")
            ok += 1
        except RuntimeError as exc:
            print(f"   ✗ {r['name']} → {exc}")
            failed += 1
        time.sleep(0.7)

    print(f"\n{ok} supprimé(s), {failed} échec(s).")


if __name__ == "__main__":
    main()
