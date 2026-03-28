const jwt = require("jsonwebtoken");
const config = require("../config");

function authenticate(req, res, next) {
  const h = req.headers.authorization;
  if (!h || !h.startsWith("Bearer ")) return res.status(401).json({ error: "Token manquant" });
  try {
    req.user = jwt.verify(h.split(" ")[1], config.jwtSecret);
    next();
  } catch (e) {
    res.status(401).json({ error: "Token invalide" });
  }
}
module.exports = { authenticate };
