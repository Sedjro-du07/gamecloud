const express = require("express");
const { PrismaClient } = require("@prisma/client");
const { authenticate } = require("../middleware/auth");
const { requireRole } = require("../middleware/rbac");
const router = express.Router();
const prisma = new PrismaClient();

router.get("/analytics", authenticate, requireRole("ADMIN"), async (req, res) => {
  const [totalUsers, totalActivities, totalSkills, xpAgg, byTrack, byRole] = await Promise.all([
    prisma.user.count(), prisma.activity.count(), prisma.userSkill.count(),
    prisma.user.aggregate({ _sum: { xp: true } }),
    prisma.user.groupBy({ by: ["track"], _count: true }),
    prisma.user.groupBy({ by: ["role"], _count: true }),
  ]);
  res.json({ totalUsers, totalActivities, totalSkills, totalXP: xpAgg._sum.xp || 0, byTrack, byRole });
});

router.get("/audit", authenticate, requireRole("PRESIDENT"), async (req, res) => {
  const logs = await prisma.auditLog.findMany({
    take: parseInt(req.query.limit) || 50,
    orderBy: { createdAt: "desc" },
    include: { user: { select: { name: true, email: true } } },
  });
  res.json(logs);
});

module.exports = router;
