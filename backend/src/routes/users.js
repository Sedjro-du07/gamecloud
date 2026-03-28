const express = require("express");
const bcrypt = require("bcryptjs");
const { PrismaClient } = require("@prisma/client");
const { authenticate } = require("../middleware/auth");
const { requireRole, ROLE_LEVEL } = require("../middleware/rbac");
const { logAudit } = require("../middleware/audit");
const config = require("../config");
const router = express.Router();
const prisma = new PrismaClient();

router.get("/", authenticate, async (req, res) => {
  const users = await prisma.user.findMany({
    select: { id:true, name:true, email:true, role:true, track:true, level:true, xp:true, badges:true, createdAt:true, _count:{select:{skills:true}} },
    orderBy: { xp: "desc" },
  });
  res.json(users);
});

router.get("/:id", authenticate, async (req, res) => {
  const user = await prisma.user.findUnique({
    where: { id: req.params.id },
    include: { skills: { include: { skill:true, validator:{select:{name:true}} } } },
  });
  if (!user) return res.status(404).json({ error: "Non trouve" });
  const { password: _, ...safe } = user;
  res.json(safe);
});

router.put("/:id", authenticate, async (req, res) => {
  if (req.user.role === "MEMBER" && req.user.id !== req.params.id) return res.status(403).json({ error: "Acces refuse" });
  const data = {};
  if (req.body.name) data.name = req.body.name;
  if (req.body.bio !== undefined) data.bio = req.body.bio;
  if (req.body.track && ROLE_LEVEL[req.user.role] >= 2) data.track = req.body.track;
  if (req.body.password) data.password = await bcrypt.hash(req.body.password, config.bcryptRounds);
  const user = await prisma.user.update({ where: { id: req.params.id }, data });
  const { password: _, ...safe } = user;
  res.json(safe);
});

router.put("/:id/role", authenticate, requireRole("PRESIDENT"), async (req, res) => {
  const { role } = req.body;
  if (!["MEMBER","ADMIN","VP","PRESIDENT"].includes(role)) return res.status(400).json({ error: "Role invalide" });
  if (req.params.id === req.user.id && role !== "PRESIDENT") return res.status(400).json({ error: "Impossible" });
  const user = await prisma.user.update({ where: { id: req.params.id }, data: { role } });
  await logAudit(req.user.id, "CHANGE_ROLE", req.params.id, role);
  const { password: _, ...safe } = user;
  res.json(safe);
});

router.delete("/:id", authenticate, requireRole("VP"), async (req, res) => {
  const target = await prisma.user.findUnique({ where: { id: req.params.id } });
  if (!target) return res.status(404).json({ error: "Non trouve" });
  if (ROLE_LEVEL[target.role] >= ROLE_LEVEL[req.user.role]) return res.status(403).json({ error: "Rang insuffisant" });
  await prisma.user.delete({ where: { id: req.params.id } });
  await logAudit(req.user.id, "EXPEL_USER", req.params.id, target.name);
  res.json({ message: "Expulse" });
});

module.exports = router;
