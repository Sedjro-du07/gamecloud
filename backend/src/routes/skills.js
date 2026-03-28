const express = require("express");
const { PrismaClient } = require("@prisma/client");
const { authenticate } = require("../middleware/auth");
const { requireRole } = require("../middleware/rbac");
const { logAudit } = require("../middleware/audit");
const router = express.Router();
const prisma = new PrismaClient();

router.get("/", authenticate, async (req, res) => {
  const skills = await prisma.skill.findMany({ include: { prereqs: { select: { prereqId: true } } }, orderBy: { block: "asc" } });
  res.json(skills.map(s => ({ id:s.id, name:s.name, block:s.block, prereqs:s.prereqs.map(p=>p.prereqId) })));
});

router.get("/user/:id", authenticate, async (req, res) => {
  const us = await prisma.userSkill.findMany({ where: { userId: req.params.id }, include: { skill:true, validator:{select:{name:true}} } });
  res.json(us);
});

router.post("/validate", authenticate, requireRole("VP"), async (req, res) => {
  try {
    const { userId, skillId, level, notes } = req.body;
    if (!userId || !skillId) return res.status(400).json({ error: "userId et skillId requis" });
    const skill = await prisma.skill.findUnique({ where: { id: skillId }, include: { prereqs: { select: { prereqId: true } } } });
    if (!skill) return res.status(404).json({ error: "Skill non trouve" });
    const user = await prisma.user.findUnique({ where: { id: userId } });
    if (!user) return res.status(404).json({ error: "User non trouve" });
    const existing = await prisma.userSkill.findMany({ where: { userId }, select: { skillId: true } });
    const has = existing.map(s => s.skillId);
    const missing = skill.prereqs.map(p => p.prereqId).filter(p => !has.includes(p));
    if (missing.length) return res.status(400).json({ error: "Prereqs manquants", missing });
    const us = await prisma.userSkill.upsert({
      where: { userId_skillId: { userId, skillId } },
      update: { level: level || "TEK1", validatedBy: req.user.id, notes },
      create: { userId, skillId, level: level || "TEK1", validatedBy: req.user.id, notes },
    });
    await prisma.user.update({ where: { id: userId }, data: { xp: { increment: 100 } } });
    await logAudit(req.user.id, "VALIDATE_SKILL", userId, skill.name);
    res.json(us);
  } catch (e) { console.error(e); res.status(500).json({ error: "Erreur serveur" }); }
});

router.delete("/revoke", authenticate, requireRole("VP"), async (req, res) => {
  const { userId, skillId } = req.body;
  await prisma.userSkill.delete({ where: { userId_skillId: { userId, skillId } } });
  await logAudit(req.user.id, "REVOKE_SKILL", userId, skillId);
  res.json({ message: "Revoque" });
});

module.exports = router;
