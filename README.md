# 🎮 GameCloud Intranet

Plateforme intranet complete pour l'association GameCloud (Epitech Benin).
Backend API + Frontend Next.js avec auth, RBAC, skills tree, activites.

## Quick Start

### 1. Base de donnees
```bash
psql -U postgres -c "CREATE DATABASE gamecloud;"
```

### 2. Backend
```bash
cd backend
npm install
cp .env.example .env
# Editez .env avec votre mot de passe PostgreSQL
npx prisma migrate dev --name init
node prisma/seed.js
npm run dev
# → http://localhost:3001
```

### 3. Frontend
```bash
cd frontend
npm install
npm run dev
# → http://localhost:3000
```

### Comptes de test (mdp: gamecloud2026)
| Email | Role |
|---|---|
| president@gamecloud.bj | PRESIDENT |
| vp@gamecloud.bj | VP |
| admin@gamecloud.bj | ADMIN |
| membre1@gamecloud.bj | MEMBER |

## RBAC

| Action | MEMBER | ADMIN | VP | PRESIDENT |
|---|---|---|---|---|
| Voir contenu | ✅ | ✅ | ✅ | ✅ |
| Creer activites | ❌ | ✅ | ✅ | ✅ |
| Valider skills | ❌ | ❌ | ✅ | ✅ |
| Expulser membre | ❌ | ❌ | ✅ | ✅ |
| Modifier roles | ❌ | ❌ | ❌ | ✅ |
| Audit log | ❌ | ❌ | ❌ | ✅ |

## Stack
- **Frontend**: Next.js 14 + Tailwind CSS
- **Backend**: Node.js + Express + Prisma
- **BDD**: PostgreSQL
- **Auth**: JWT + bcrypt
