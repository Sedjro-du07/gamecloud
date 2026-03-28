const express = require("express");
const bcrypt = require("bcryptjs");
const jwt = require("jsonwebtoken");
const { PrismaClient } = require("@prisma/client");
const config = require("../config");
const { authenticate } = require("../middleware/auth");
const { requireRole } = require("../middleware/rbac");
const { logAudit } = require("../middleware/audit");
const router = express.Router();
const prisma = new PrismaClient();

router.post("/login", async (req, res) => {
  try {
    const { email, password } = req.body;
    if (!email || !password) return res.status(400).json({ error: "Email et mot de passe requis" });
    const user = await prisma.user.findUnique({ where: { email } });
    if (!user || !(await bcrypt.compare(password, user.password))) {
      return res.status(401).json({ error: "Identifiants incorrects" });
    }
    const payload = { id: user.id, email: user.email, role: user.role, name: user.name };
    const token = jwt.sign(payload, config.jwtSecret, { expiresIn: config.jwtExpiry });
    const refreshToken = jwt.sign({ id: user.id }, config.jwtRefreshSecret, { expiresIn: config.jwtRefreshExpiry });
    await logAudit(user.id, "LOGIN", null, null);
    const { password: _, ...safe } = user;
    res.json({ token, refreshToken, user: safe });
  } catch (e) { console.error(e); res.status(500).json({ error: "Erreur serveur" }); }
});

router.post("/register", authenticate, requireRole("ADMIN"), async (req, res) => {
  try {
    const { email, password, name, role, track } = req.body;
    if (!email || !password || !name) return res.status(400).json({ error: "Champs requis" });
    if (await prisma.user.findUnique({ where: { email } })) return res.status(409).json({ error: "Email pris" });
    if ((role === "VP" || role === "PRESIDENT") && req.user.role !== "PRESIDENT") {
      return res.status(403).json({ error: "Seul le president peut creer VP/President" });
    }
    const user = await prisma.user.create({
      data: { email, password: await bcrypt.hash(password, config.bcryptRounds), name, role: role || "MEMBER", track: track || "ENGINEERING" },
    });
    await logAudit(req.user.id, "CREATE_USER", user.id, email);
    const { password: _, ...safe } = user;
    res.status(201).json(safe);
  } catch (e) { console.error(e); res.status(500).json({ error: "Erreur serveur" }); }
});

router.get("/me", authenticate, async (req, res) => {
  try {
    const user = await prisma.user.findUnique({
      where: { id: req.user.id },
      include: { skills: { include: { skill: true } } },
    });
    if (!user) return res.status(404).json({ error: "Non trouve" });
    const { password: _, ...safe } = user;
    res.json(safe);
  } catch (e) { res.status(500).json({ error: "Erreur serveur" }); }
});

router.post("/refresh", async (req, res) => {
  try {
    const { refreshToken } = req.body;
    const decoded = jwt.verify(refreshToken, config.jwtRefreshSecret);
    const user = await prisma.user.findUnique({ where: { id: decoded.id } });
    if (!user) return res.status(401).json({ error: "Invalide" });
    const payload = { id: user.id, email: user.email, role: user.role, name: user.name };
    res.json({ token: jwt.sign(payload, config.jwtSecret, { expiresIn: config.jwtExpiry }) });
  } catch (e) { res.status(401).json({ error: "Refresh invalide" }); }
});

module.exports = router;
