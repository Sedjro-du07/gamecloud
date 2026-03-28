"use client";
import { useEffect, useState } from "react";
import { api, getUser, getRoleLevel } from "@/lib/api";

const TC = { ENGINEERING: "#00d4ff", DESIGN: "#ff00aa", AUDIO: "#ff8800", VISUALS: "#8844ff" };

export default function ValidationPage() {
  const [members, setMembers] = useState([]);
  const [skills, setSkills] = useState([]);
  const [selected, setSelected] = useState(null);
  const [memberSkills, setMemberSkills] = useState([]);
  const [msg, setMsg] = useState("");
  const user = getUser();

  useEffect(() => {
    api("/users").then(setMembers);
    api("/skills").then(setSkills);
  }, []);

  if (!user || getRoleLevel(user.role) < 3) {
    return (
      <div className="flex items-center justify-center h-96">
        <div className="text-center">
          <div className="text-4xl mb-3">🔒</div>
          <div className="text-gc-muted">Acces reserve aux VP et Presidents</div>
        </div>
      </div>
    );
  }

  async function selectMember(m) {
    setSelected(m);
    setMsg("");
    const us = await api("/skills/user/" + m.id);
    setMemberSkills(us.map(s => s.skillId));
  }

  async function validate(skillId) {
    try {
      await api("/skills/validate", { method: "POST", body: JSON.stringify({ userId: selected.id, skillId }) });
      setMsg("Skill valide !");
      const us = await api("/skills/user/" + selected.id);
      setMemberSkills(us.map(s => s.skillId));
    } catch (e) {
      setMsg("Erreur: " + e.message);
    }
  }

  const others = members.filter(m => m.id !== user.id);

  return (
    <div>
      <h2 className="text-xl font-bold text-gc-text mb-1">✅ Validation de Competences</h2>
      <p className="text-xs text-gc-muted mb-5">Selectionnez un membre puis validez ses skills</p>

      {msg && <div className="mb-4 p-3 rounded-lg bg-gc-neon/10 border border-gc-neon/30 text-gc-neon text-sm font-mono">{msg}</div>}

      <div className="grid grid-cols-[240px_1fr] gap-4">
        {/* Member list */}
        <div className="space-y-1.5">
          <div className="text-[10px] text-gc-muted font-mono tracking-wider mb-2">MEMBRES</div>
          {others.map(m => (
            <button key={m.id} onClick={() => selectMember(m)}
              className="w-full text-left p-2.5 rounded-lg border transition-all"
              style={{
                background: selected?.id === m.id ? "rgba(0,255,136,0.08)" : "#0c0c14",
                borderColor: selected?.id === m.id ? "rgba(0,255,136,0.4)" : "#1a1a30",
              }}>
              <div className="text-sm font-semibold text-gc-text">{m.name}</div>
              <div className="flex gap-1 mt-1">
                <span className="px-1.5 py-0.5 rounded text-[9px] font-mono" style={{ background: TC[m.track]+"18", color: TC[m.track] }}>{m.track}</span>
                <span className="px-1.5 py-0.5 rounded text-[9px] font-mono bg-gc-neon/10 text-gc-neon">{m.level}</span>
              </div>
            </button>
          ))}
        </div>

        {/* Skills grid */}
        <div>
          {selected ? (
            <>
              <div className="text-sm font-bold text-gc-text mb-3">Skills de {selected.name}</div>
              <div className="grid grid-cols-4 gap-2">
                {skills.map(skill => {
                  const has = memberSkills.includes(skill.id);
                  const canUnlock = !has && skill.prereqs.every(p => memberSkills.includes(p));
                  const color = has ? "#00ff88" : canUnlock ? "#00d4ff" : "#3a3a55";

                  return (
                    <div key={skill.id} className="bg-gc-card border rounded-lg p-3 text-center"
                      style={{ borderColor: color + "30", opacity: has ? 1 : canUnlock ? 0.9 : 0.35 }}>
                      <div className="text-[11px] font-mono font-bold" style={{ color }}>{skill.name}</div>
                      <div className="text-[9px] text-gc-dim my-1">Block {skill.block}</div>
                      {has ? (
                        <span className="text-[9px] font-mono font-bold px-2 py-0.5 rounded bg-gc-neon/15 text-gc-neon border border-gc-neon/30">VALIDE</span>
                      ) : canUnlock ? (
                        <button onClick={() => validate(skill.id)}
                          className="text-[9px] font-mono font-bold px-3 py-1 rounded bg-gc-cyan/15 text-gc-cyan border border-gc-cyan/30 hover:bg-gc-cyan/25 transition-all cursor-pointer">
                          VALIDER
                        </button>
                      ) : (
                        <span className="text-[9px] text-gc-dim">prereqs</span>
                      )}
                    </div>
                  );
                })}
              </div>
            </>
          ) : (
            <div className="flex items-center justify-center h-64 bg-gc-card border border-gc-border rounded-lg">
              <div className="text-center">
                <div className="text-3xl mb-2">👈</div>
                <div className="text-sm text-gc-muted">Selectionnez un membre</div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
