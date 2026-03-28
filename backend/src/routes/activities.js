const express = require("express");
const { PrismaClient } = require("@prisma/client");
const { authenticate } = require("../middleware/auth");
const { requireRole } = require("../middleware/rbac");
const { logAudit } = require("../middleware/audit");
const router = express.Router();
const prisma = new PrismaClient();

router.get("/", authenticate, async (req, res) => {
  const where = {};
  if (req.query.type) where.type = req.query.type;
  if (req.query.track) where.track = req.query.track;
  const acts = await prisma.activity.findMany({ where, include: { creator: { select: { name: true } } }, orderBy: { date: "asc" } });
  res.json(acts);
});

router.get("/:id", authenticate, async (req, res) => {
  const a = await prisma.activity.findUnique({ where: { id: req.params.id }, include: { creator: { select: { name: true } } } });
  if (!a) return res.status(404).json({ error: "Non trouve" });
  res.json(a);
});

router.post("/", authenticate, requireRole("ADMIN"), async (req, res) => {
  const { type, title, description, date, duration, track, level } = req.body;
  if (!type || !title || !date) return res.status(400).json({ error: "Type, titre, date requis" });
  const a = await prisma.activity.create({ data: { type, title, description: description||"", date: new Date(date), duration: duration||"1h", track: track||"all", level: level||"all", createdBy: req.user.id } });
  await logAudit(req.user.id, "CREATE_ACTIVITY", a.id, title);
  res.status(201).json(a);
});

router.put("/:id", authenticate, requireRole("ADMIN"), async (req, res) => {
  const data = {};
  ["type","title","description","duration","track","level","status"].forEach(f => { if (req.body[f] !== undefined) data[f] = req.body[f]; });
  if (req.body.date) data.date = new Date(req.body.date);
  const a = await prisma.activity.update({ where: { id: req.params.id }, data });
  res.json(a);
});

router.delete("/:id", authenticate, requireRole("VP"), async (req, res) => {
  const a = await prisma.activity.findUnique({ where: { id: req.params.id } });
  if (!a) return res.status(404).json({ error: "Non trouve" });
  await prisma.activity.delete({ where: { id: req.params.id } });
  await logAudit(req.user.id, "DELETE_ACTIVITY", req.params.id, a.title);
  res.json({ message: "Supprime" });
});

module.exports = router;
