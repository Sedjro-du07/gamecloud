const ROLE_LEVEL = { MEMBER: 1, ADMIN: 2, VP: 3, PRESIDENT: 4 };

function requireRole(minRole) {
  return (req, res, next) => {
    if (!req.user) return res.status(401).json({ error: "Non authentifie" });
    if ((ROLE_LEVEL[req.user.role] || 0) < (ROLE_LEVEL[minRole] || 999)) {
      return res.status(403).json({ error: "Acces refuse", required: minRole, current: req.user.role });
    }
    next();
  };
}
module.exports = { requireRole, ROLE_LEVEL };
