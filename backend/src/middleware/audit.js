const { PrismaClient } = require("@prisma/client");
const prisma = new PrismaClient();

async function logAudit(userId, action, target, details) {
  try { await prisma.auditLog.create({ data: { userId, action, target, details } }); } catch (e) { console.error("Audit:", e.message); }
}
module.exports = { logAudit };
