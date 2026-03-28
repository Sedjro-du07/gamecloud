const { PrismaClient } = require("@prisma/client");
const bcrypt = require("bcryptjs");
const prisma = new PrismaClient();

async function main() {
  console.log("Seeding...");
  const hash = await bcrypt.hash("gamecloud2026", 12);

  const skills = [
    { id: "io", name: "Input/Output", block: 1 },
    { id: "cond", name: "Conditions", block: 1 },
    { id: "func", name: "Functions", block: 2 },
    { id: "arr", name: "Arrays", block: 2 },
    { id: "ptr", name: "Pointers", block: 3 },
    { id: "mem", name: "Dynamic Memory", block: 3 },
    { id: "file", name: "File I/O", block: 4 },
    { id: "mod", name: "Modules", block: 4 },
    { id: "loop", name: "Game Loop", block: 5 },
    { id: "input", name: "Input Handling", block: 5 },
    { id: "coll", name: "Collisions", block: 6 },
    { id: "state", name: "State Management", block: 6 },
    { id: "ai", name: "Enemy AI", block: 7 },
    { id: "score", name: "Score System", block: 7 },
    { id: "ui", name: "UI Systems", block: 8 },
    { id: "fb", name: "Feedback", block: 8 },
  ];
  for (const s of skills) {
    await prisma.skill.upsert({ where: { id: s.id }, update: {}, create: s });
  }

  const prereqs = [
    ["func","io"],["func","cond"],["arr","io"],["ptr","func"],["ptr","arr"],
    ["mem","ptr"],["file","func"],["mod","file"],["mod","mem"],["loop","mod"],
    ["input","loop"],["coll","loop"],["state","coll"],["ai","state"],
    ["score","state"],["ui","score"],["fb","ui"],["fb","ai"],
  ];
  for (const [s, p] of prereqs) {
    await prisma.skillPrereq.upsert({
      where: { skillId_prereqId: { skillId: s, prereqId: p } },
      update: {}, create: { skillId: s, prereqId: p },
    });
  }

  const users = [
    { email: "president@gamecloud.bj", name: "Joachim K.", role: "PRESIDENT", track: "ENGINEERING", level: "MASTER", xp: 2400, badges: 12 },
    { email: "vp@gamecloud.bj", name: "Ama D.", role: "VP", track: "DESIGN", level: "TEK3", xp: 1800, badges: 8 },
    { email: "admin@gamecloud.bj", name: "Kofi M.", role: "ADMIN", track: "AUDIO", level: "TEK2", xp: 1200, badges: 5 },
    { email: "membre1@gamecloud.bj", name: "Fatou S.", role: "MEMBER", track: "VISUALS", level: "TEK2", xp: 900, badges: 3 },
    { email: "membre2@gamecloud.bj", name: "Yao B.", role: "MEMBER", track: "ENGINEERING", level: "TEK1", xp: 450, badges: 2 },
  ];
  for (const u of users) {
    await prisma.user.upsert({ where: { email: u.email }, update: {}, create: { ...u, password: hash } });
  }

  const pres = await prisma.user.findUnique({ where: { email: "president@gamecloud.bj" } });

  const skillSets = {
    "president@gamecloud.bj": skills.map(s => s.id),
    "vp@gamecloud.bj": ["io","cond","func","arr","ptr","mem","file","mod","loop","input"],
    "admin@gamecloud.bj": ["io","cond","func","arr","ptr","mem"],
    "membre1@gamecloud.bj": ["io","cond","func","arr","ptr"],
    "membre2@gamecloud.bj": ["io","cond","func"],
  };
  for (const [email, sids] of Object.entries(skillSets)) {
    const user = await prisma.user.findUnique({ where: { email } });
    for (const sid of sids) {
      await prisma.userSkill.upsert({
        where: { userId_skillId: { userId: user.id, skillId: sid } },
        update: {}, create: { userId: user.id, skillId: sid, validatedBy: pres.id },
      });
    }
  }

  const acts = [
    { type: "SESSION", title: "Workshop Raylib Basics", description: "Introduction Raylib : fenetre, formes, input", date: new Date("2026-04-07"), duration: "2h", track: "engineering", level: "Tek1" },
    { type: "SESSION", title: "Prototypage Papier", description: "Techniques de prototypage rapide", date: new Date("2026-04-07"), duration: "2h", track: "design", level: "Tek1" },
    { type: "DEFENSE", title: "Defense Block 1", description: "Presentation micro-projets Block 1", date: new Date("2026-04-14"), duration: "1h30", track: "all", level: "all" },
    { type: "CONFERENCE", title: "Game Dev en Afrique", description: "Ecosysteme game dev africain", date: new Date("2026-04-21"), duration: "1h", track: "all", level: "all" },
    { type: "GAMEJAM", title: "Mini Game Jam #1", description: "48h sur le theme LOOP", date: new Date("2026-05-02"), duration: "48h", track: "all", level: "all" },
    { type: "EVALUATION", title: "Evaluation Tek1 > Tek2", description: "Passage de niveau", date: new Date("2026-05-12"), duration: "30min/pers", track: "all", level: "Tek1" },
  ];
  for (const a of acts) {
    await prisma.activity.create({ data: { ...a, createdBy: pres.id } });
  }

  console.log("Done! Comptes: president/vp/admin/membre1@gamecloud.bj (mdp: gamecloud2026)");
}

main().catch(e => { console.error(e); process.exit(1); }).finally(() => prisma.$disconnect());
