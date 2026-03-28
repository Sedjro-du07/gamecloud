"use client";
import { useEffect, useState } from "react";
import { api, getUser } from "@/lib/api";

const ROLE_COLORS = { PRESIDENT: "#ffd700", VP: "#ff00aa", ADMIN: "#00d4ff", MEMBER: "#00ff88" };
const TRACK_COLORS = { ENGINEERING: "#00d4ff", DESIGN: "#ff00aa", AUDIO: "#ff8800", VISUALS: "#8844ff" };
const ACT_ICONS = { SESSION: "📖", DEFENSE: "🎤", EVALUATION: "📝", CONFERENCE: "🎙️", GAMEJAM: "🕹️" };
const ACT_COLORS = { SESSION: "#00ff88", DEFENSE: "#ff00aa", EVALUATION: "#ff8800", CONFERENCE: "#00d4ff", GAMEJAM: "#8844ff" };

function Badge({ children, color }) {
  return <span className="inline-block px-2 py-0.5 rounded text-[10px] font-mono font-bold" style={{ background: color + "20", color, border: "1px solid " + color + "30" }}>{children}</span>;
}

function StatCard({ icon, value, label, color }) {
  return (
    <div className="bg-gc-card border border-gc-border rounded-lg p-4 text-center">
      <div className="text-xl mb-1">{icon}</div>
      <div className="text-xl font-bold font-mono glow-green" style={{ color }}>{value}</div>
      <div className="text-[10px] text-gc-muted font-mono mt-1">{label}</div>
    </div>
  );
}

export default function DashboardPage() {
  const [me, setMe] = useState(null);
  const [acts, setActs] = useState([]);
  const user = getUser();

  useEffect(() => {
    api("/auth/me").then(setMe).catch(() => {});
    api("/activities").then(a => setActs(a.slice(0, 5))).catch(() => {});
  }, []);

  if (!user) return null;
  const skillCount = me?.skills?.length || 0;
  const pct = Math.round((skillCount / 16) * 100);

  return (
    <div>
      {/* Hero */}
      <div className="relative rounded-xl p-7 mb-6 overflow-hidden border border-gc-neon/20" style={{ background: "linear-gradient(135deg, #0c0c14, rgba(0,255,136,0.05))" }}>
        <div className="text-[10px] text-gc-muted font-mono tracking-[4px] mb-1">BIENVENUE</div>
        <h1 className="text-2xl font-bold text-gc-text mb-2">{user.name}</h1>
        <div className="flex gap-2">
          <Badge color={TRACK_COLORS[user.track]}>{user.track}</Badge>
          <Badge color={ROLE_COLORS[user.role]}>{user.role}</Badge>
          <Badge color="#ffd700">LVL {user.level}</Badge>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-4 gap-3 mb-6">
        <StatCard icon="⚡" value={user.xp} label="XP TOTAL" color="#00ff88" />
        <StatCard icon="🧬" value={skillCount + "/16"} label="SKILLS" color="#00d4ff" />
        <StatCard icon="🏅" value={user.badges} label="BADGES" color="#ffd700" />
        <StatCard icon="📊" value={pct + "%"} label="COMPLETION" color="#ff00aa" />
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* Activities */}
        <div>
          <h3 className="text-sm font-bold text-gc-text mb-3">📅 Prochaines activites</h3>
          <div className="space-y-2">
            {acts.map(a => (
              <div key={a.id} className="bg-gc-card border border-gc-border rounded-lg p-3">
                <div className="flex gap-2 mb-1">
                  <Badge color={ACT_COLORS[a.type]}>{ACT_ICONS[a.type]} {a.type}</Badge>
                  {a.level !== "all" && <Badge color="#00ff88">{a.level}</Badge>}
                </div>
                <div className="text-sm font-semibold text-gc-text">{a.title}</div>
                <div className="text-xs text-gc-muted mt-1">{new Date(a.date).toLocaleDateString("fr-FR")} | {a.duration}</div>
              </div>
            ))}
          </div>
        </div>

        {/* Progression */}
        <div>
          <h3 className="text-sm font-bold text-gc-text mb-3">🧬 Progression</h3>
          <div className="bg-gc-card border border-gc-border rounded-lg p-4">
            <div className="flex justify-between mb-2">
              <span className="text-xs text-gc-muted">Completion globale</span>
              <span className="text-xs font-mono text-gc-neon">{pct}%</span>
            </div>
            <div className="w-full h-2 bg-gc-border rounded-full overflow-hidden mb-4">
              <div className="h-full rounded-full bg-gc-neon" style={{ width: pct + "%", boxShadow: "0 0 8px rgba(0,255,136,0.5)" }} />
            </div>
            {[1,2,3,4,5,6,7,8].map(b => {
              const total = b <= 8 ? 2 : 0;
              const done = me?.skills?.filter(s => s.skill?.block === b).length || 0;
              return (
                <div key={b} className="flex items-center gap-2 mb-1.5">
                  <span className="text-[10px] text-gc-dim font-mono w-8">B{b}</span>
                  <div className="flex-1 h-1 bg-gc-border rounded-full overflow-hidden">
                    <div className="h-full rounded-full" style={{ width: (done/total*100)+"%", background: done===total?"#00ff88":"#00d4ff" }} />
                  </div>
                  <span className="text-[10px] text-gc-muted w-6 text-right">{done}/{total}</span>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
