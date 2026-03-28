"use client";
import { useEffect, useState } from "react";
import { api, getUser } from "@/lib/api";

export default function SkillsPage() {
  const [skills, setSkills] = useState([]);
  const [userSkills, setUserSkills] = useState([]);
  const user = getUser();

  useEffect(() => {
    if (!user) return;
    api("/skills").then(setSkills);
    api("/skills/user/" + user.id).then(us => setUserSkills(us.map(s => s.skillId)));
  }, []);

  return (
    <div>
      <h2 className="text-xl font-bold text-gc-text mb-5">🧬 Skills Tree</h2>

      <div className="grid grid-cols-4 gap-3">
        {skills.map(skill => {
          const unlocked = userSkills.includes(skill.id);
          const canUnlock = !unlocked && skill.prereqs.every(p => userSkills.includes(p));
          const color = unlocked ? "#00ff88" : canUnlock ? "#00d4ff" : "#3a3a55";

          return (
            <div key={skill.id} className="bg-gc-card border rounded-lg p-4 text-center transition-all"
              style={{ borderColor: color + "40", opacity: unlocked ? 1 : canUnlock ? 0.85 : 0.4, boxShadow: unlocked ? "0 0 15px rgba(0,255,136,0.15)" : "none" }}>
              <div className="w-10 h-10 rounded-full mx-auto mb-2 flex items-center justify-center text-sm"
                style={{ border: "2px solid " + color, background: unlocked ? color + "20" : "transparent", boxShadow: unlocked ? "0 0 12px " + color + "40" : "none" }}>
                {unlocked ? <span style={{ color }}>✓</span> : canUnlock ? <span style={{ color }}>◉</span> : <span style={{ color }}>○</span>}
              </div>
              <div className="text-xs font-mono font-bold" style={{ color }}>{skill.name}</div>
              <div className="text-[10px] text-gc-dim mt-1">Block {skill.block}</div>
              {unlocked && <span className="inline-block mt-2 px-2 py-0.5 rounded text-[9px] font-mono font-bold bg-gc-neon/15 text-gc-neon border border-gc-neon/30">VALIDE</span>}
              {canUnlock && <span className="inline-block mt-2 px-2 py-0.5 rounded text-[9px] font-mono font-bold bg-gc-cyan/15 text-gc-cyan border border-gc-cyan/30">DISPONIBLE</span>}
            </div>
          );
        })}
      </div>

      <div className="bg-gc-card border border-gc-border rounded-lg p-4 mt-5">
        <span className="text-xs text-gc-muted font-mono">
          LEGENDE: <span className="text-gc-neon">● Valide</span> | <span className="text-gc-cyan">● Disponible</span> | <span className="text-gc-dim">● Verrouille</span>
        </span>
      </div>
    </div>
  );
}
