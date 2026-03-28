const express = require("express");
const cors = require("cors");
const morgan = require("morgan");
const rateLimit = require("express-rate-limit");
const config = require("./config");

const app = express();
app.use(cors({ origin: config.corsOrigin, credentials: true }));
app.use(express.json());
app.use(morgan("dev"));

app.use("/api/auth", rateLimit({ windowMs: 15*60*1000, max: 20 }), require("./routes/auth"));
app.use("/api/users", require("./routes/users"));
app.use("/api/skills", require("./routes/skills"));
app.use("/api/activities", require("./routes/activities"));
app.use("/api/admin", require("./routes/admin"));

app.get("/api/health", (_, res) => res.json({ status: "ok" }));
app.use((_, res) => res.status(404).json({ error: "Not found" }));

app.listen(config.port, () => {
  console.log("\n  🎮 GameCloud API → http://localhost:" + config.port + "\n");
});
