"use client";
import { useEffect, useState } from "react";
import { api, getUser, getRoleLevel } from "@/lib/api";

const RC = { PRESIDENT: "#ffd700", VP: "#ff00aa", ADMIN: "#00d4ff", MEMBER: "#00ff88" };
const RI = { PRESIDENT: "👑", VP: "⚡", ADMIN: "🛡️", MEMBER: "🎮" };
const AC = { SESSION: "#00ff88", DEFENSE: "#ff00aa", EVALUATION: "#ff8800", CONFERENCE: "#00d4ff", GAMEJAM: "#8844ff" };
const TC = { ENGINEERING: "#00d4ff", DESIGN: "#ff00aa", AUDIO: "#ff8800", VISUALS: "#8844ff" };

export default function AdminPage() {
  const [tab, setTab] = useState("users");
  const [members, setMembers] = useState([]);
  const [acts, setActs] = useState([]);
  const [analytics, setAnalytics] = useState(null);
  const [audit, setAudit] = useState([]);
  const user = getUser();
  const level = getRoleLevel(user?.role);

  useEffect(() => {
    if (level < 2) return;
    api("/users").then(setMembers);
    api("/activities").then(setActs);
    api("/admin/analytics").then(setAnalytics).catch(() => {});
    if (level >= 4) api("/admin/audit").then(setAudit).catch(() => {});
  }, []);

  if (!user || level < 2) {
    return <div className="flex items-center justify-center h-96"><div className="text-4xl">🔒</div></div>;
  }

  const tabs = [{ id: "users", label: "👥 Utilisateurs" }, { id: "activities", label: "📅 Activites" }, { id: "analytics", label: "📊 Analytics" }];
  if (level >= 4) tabs.push({ id: "audit", label: "📋 Audit Log" });
  if (level >= 3) tabs.push({ id: "rbac", label: "🛡️ RBAC" });

  async function handleExpel(id, name) {
    if (!confirm("Expulser " + name + " ?")) return;
    try {
      await api("/users/" + id, { method: "DELETE" });
      setMembers(members.filter(m => m.id !== id));
    } catch (e) { alert(e.message); }
  }

  async function handleDeleteAct(id) {
    if (!confirm("Supprimer cette activite ?")) return;
    try {
      await api("/activities/" + id, { method: "DELETE" });
      setActs(acts.filter(a => a.id !== id));
    } catch (e) { alert(e.message); }
  }

  return (
    <div>
      <h2 className="text-xl font-bold text-gc-text mb-4">⚙️ Administration</h2>

      <div className="flex gap-2 mb-5 flex-wrap">
        {tabs.map(t => (
          <button key={t.id} onClick={() => setTab(t.id)}
            className="px-4 py-2 rounded-md text-xs font-mono font-semibold transition-all"
            style={{ background: tab === t.id ? "rgba(0,255,136,0.1)" : "transparent", border: "1px solid " + (tab === t.id ? "rgba(0,255,136,0.3)" : "#1a1a30"), color: tab === t.id ? "#00ff88" : "#6a6a88" }}>
            {t.label}
          </button>
        ))}
      </div>

      {/* Users tab */}
      {tab === "users" && (
        <div className="space-y-2">
          <div className="text-xs text-gc-muted mb-2">{members.length} membres</div>
          {members.map(m => (
            <div key={m.id} className="bg-gc-card border border-gc-border rounded-lg p-3 flex items-center gap-3">
              <span className="text-lg">{RI[m.role]}</span>
              <div className="flex-1">
                <div className="text-sm font-semibold text-gc-text">{m.name}</div>
                <div className="flex gap-1 mt-1">
                  <span className="px-1.5 py-0.5 rounded text-[9px] font-mono" style={{ background: RC[m.role]+"18", color: RC[m.role] }}>{m.role}</span>
                  <span className="px-1.5 py-0.5 rounded text-[9px] font-mono" style={{ background: TC[m.track]+"18", color: TC[m.track] }}>{m.track}</span>
                </div>
              </div>
              <div className="text-xs text-gc-muted font-mono">{m.email}</div>
              {level >= 3 && m.role !== "PRESIDENT" && m.id !== user.id && (
                <button onClick={() => handleExpel(m.id, m.name)}
                  className="px-2.5 py-1 rounded text-[10px] font-mono font-bold text-red-400 border border-red-400/30 hover:bg-red-400/10">
                  EXPULSER
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Activities tab */}
      {tab === "activities" && (
        <div className="space-y-2">
          <div className="text-xs text-gc-muted mb-2">{acts.length} activites</div>
          {acts.map(a => (
            <div key={a.id} className="bg-gc-card border border-gc-border rounded-lg p-3 flex items-center gap-3">
              <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold" style={{ background: AC[a.type]+"20", color: AC[a.type] }}>{a.type}</span>
              <div className="flex-1">
                <div className="text-sm font-semibold text-gc-text">{a.title}</div>
                <div className="text-[10px] text-gc-muted">{new Date(a.date).toLocaleDateString("fr-FR")}</div>
              </div>
              {level >= 3 && (
                <button onClick={() => handleDeleteAct(a.id)}
                  className="px-2.5 py-1 rounded text-[10px] font-mono font-bold text-red-400 border border-red-400/30 hover:bg-red-400/10">
                  SUPPR
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Analytics tab */}
      {tab === "analytics" && analytics && (
        <div>
          <div className="grid grid-cols-4 gap-3 mb-5">
            {[
              { l: "Membres", v: analytics.totalUsers, c: "#00ff88" },
              { l: "Activites", v: analytics.totalActivities, c: "#00d4ff" },
              { l: "Skills valides", v: analytics.totalSkills, c: "#ff00aa" },
              { l: "XP total", v: analytics.totalXP, c: "#ffd700" },
            ].map((s, i) => (
              <div key={i} className="bg-gc-card border border-gc-border rounded-lg p-5 text-center">
                <div className="text-2xl font-bold font-mono" style={{ color: s.c }}>{s.v}</div>
                <div className="text-[10px] text-gc-muted font-mono mt-1">{s.l}</div>
              </div>
            ))}
          </div>
          <div className="bg-gc-card border border-gc-border rounded-lg p-4">
            <div className="text-sm font-bold text-gc-text mb-3">Par Track</div>
            {analytics.byTrack.map(t => (
              <div key={t.track} className="flex items-center gap-3 mb-2">
                <span className="text-xs w-24 font-mono" style={{ color: TC[t.track] }}>{t.track}</span>
                <div className="flex-1 h-2 bg-gc-border rounded-full overflow-hidden">
                  <div className="h-full rounded-full" style={{ width: (t._count/analytics.totalUsers*100)+"%", background: TC[t.track] }} />
                </div>
                <span className="text-xs text-gc-muted w-6 text-right">{t._count}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Audit tab */}
      {tab === "audit" && (
        <div className="space-y-1.5">
          {audit.map(log => (
            <div key={log.id} className="bg-gc-card border border-gc-border rounded-lg p-3 flex justify-between"
              style={{ borderLeftWidth: 3, borderLeftColor: log.action.includes("EXPEL") ? "#ff3355" : log.action.includes("ROLE") ? "#ffd700" : log.action.includes("VALID") ? "#00ff88" : "#00d4ff" }}>
              <div>
                <span className="text-xs font-semibold text-gc-text">{log.user.name}</span>
                <span className="text-xs text-gc-muted"> — {log.action}</span>
                {log.details && <span className="text-xs text-gc-dim"> ({log.details})</span>}
              </div>
              <span className="text-[10px] text-gc-dim font-mono shrink-0">{new Date(log.createdAt).toLocaleString("fr-FR")}</span>
            </div>
          ))}
          {audit.length === 0 && <div className="text-sm text-gc-muted text-center py-10">Aucune action enregistree</div>}
        </div>
      )}

      {/* RBAC tab */}
      {tab === "rbac" && (
        <div className="bg-gc-card border border-gc-border rounded-lg p-5 overflow-x-auto">
          <div className="text-sm font-bold text-gc-text mb-3">Matrice de Permissions RBAC</div>
          <table className="w-full text-xs font-mono">
            <thead>
              <tr>
                <th className="text-left p-2 border-b border-gc-border text-gc-muted">Permission</th>
                {["MEMBER","ADMIN","VP","PRESIDENT"].map(r => <th key={r} className="p-2 border-b border-gc-border" style={{ color: RC[r] }}>{RI[r]} {r}</th>)}
              </tr>
            </thead>
            <tbody>
              {[
                ["Voir contenu", true, true, true, true],
                ["Creer activites", false, true, true, true],
                ["Valider skills", false, false, true, true],
                ["Gerer membres", false, true, true, true],
                ["Expulser", false, false, true, true],
                ["Modifier roles", false, false, false, true],
                ["Audit log", false, false, false, true],
              ].map((row, i) => (
                <tr key={i}>
                  <td className="p-2 border-b border-gc-border text-gc-text">{row[0]}</td>
                  {[row[1],row[2],row[3],row[4]].map((v, j) => (
                    <td key={j} className="p-2 border-b border-gc-border text-center" style={{ color: v ? "#00ff88" : "#ff3355" }}>{v ? "✓" : "✗"}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
