require("dotenv").config();
module.exports = {
  port: process.env.PORT || 3001,
  jwtSecret: process.env.JWT_SECRET || "dev-secret-change-me",
  jwtRefreshSecret: process.env.JWT_REFRESH_SECRET || "dev-refresh-change-me",
  jwtExpiry: "24h",
  jwtRefreshExpiry: "7d",
  corsOrigin: process.env.CORS_ORIGIN || "http://localhost:3000",
  bcryptRounds: 12,
};
