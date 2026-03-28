"use client";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";

const RC = { PRESIDENT: "#ffd700", VP: "#ff00aa", ADMIN: "#00d4ff", MEMBER: "#00ff88" };
const RI = { PRESIDENT: "👑", VP: "⚡", ADMIN: "🛡️", MEMBER: "🎮" };
const TC = { ENGINEERING: "#00d4ff", DESIGN: "#ff00aa", AUDIO: "#ff8800", VISUALS: "#8844ff" };

export default function MembersPage() {
  const [members, setMembers] = useState([]);
  useEffect(() => { api("/users").then(setMembers); }, []);

  return (
    <div>
      <h2 className="text-xl font-bold text-gc-text mb-4">👥 Membres ({members.length})</h2>
      <div className="space-y-2">
        {members.map(m => {
          const pct = Math.round(((m._count?.skills || 0) / 16) * 100);
          return (
            <div key={m.id} className="bg-gc-card border border-gc-border rounded-lg p-3 flex items-center gap-3">
              <div className="w-10 h-10 rounded-lg flex items-center justify-center text-lg shrink-0"
                style={{ background: RC[m.role] + "15", border: "1px solid " + RC[m.role] + "30" }}>
                {RI[m.role]}
              </div>
              <div className="flex-1">
                <div className="text-sm font-bold text-gc-text">{m.name}</div>
                <div className="flex gap-1.5 mt-1">
                  <span className="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold" style={{ background: RC[m.role]+"18", color: RC[m.role] }}>{m.role}</span>
                  <span className="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold" style={{ background: TC[m.track]+"18", color: TC[m.track] }}>{m.track}</span>
                  <span className="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-gc-neon/10 text-gc-neon">{m.level}</span>
                </div>
              </div>
              <div className="text-right">
                <div className="text-sm font-bold font-mono text-yellow-400">{m.xp} XP</div>
                <div className="w-24 h-1 bg-gc-border rounded-full overflow-hidden mt-1">
                  <div className="h-full rounded-full" style={{ width: pct+"%", background: TC[m.track] }} />
                </div>
                <div className="text-[9px] text-gc-muted mt-0.5">{pct}% skills</div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
