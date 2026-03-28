"use client";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";

const TYPES = ["", "SESSION", "DEFENSE", "EVALUATION", "CONFERENCE", "GAMEJAM"];
const LABELS = { SESSION: "📖 Session", DEFENSE: "🎤 Defense", EVALUATION: "📝 Evaluation", CONFERENCE: "🎙️ Conference", GAMEJAM: "🕹️ Game Jam" };
const COLORS = { SESSION: "#00ff88", DEFENSE: "#ff00aa", EVALUATION: "#ff8800", CONFERENCE: "#00d4ff", GAMEJAM: "#8844ff" };
const TRACK_COLORS = { engineering: "#00d4ff", design: "#ff00aa", audio: "#ff8800", visuals: "#8844ff" };

export default function ActivitiesPage() {
  const [acts, setActs] = useState([]);
  const [filter, setFilter] = useState("");

  useEffect(() => { api("/activities").then(setActs); }, []);

  const filtered = filter ? acts.filter(a => a.type === filter) : acts;

  return (
    <div>
      <h2 className="text-xl font-bold text-gc-text mb-4">📅 Activites GameCloud</h2>

      <div className="flex gap-2 mb-5 flex-wrap">
        {TYPES.map(t => (
          <button key={t} onClick={() => setFilter(t)}
            className="px-3 py-1.5 rounded-md text-xs font-mono font-semibold transition-all"
            style={{
              background: filter === t ? (COLORS[t] || "#00ff88") + "20" : "transparent",
              border: "1px solid " + (filter === t ? (COLORS[t] || "#00ff88") : "#1a1a30"),
              color: filter === t ? (COLORS[t] || "#00ff88") : "#6a6a88",
            }}>
            {t ? LABELS[t] : "🌐 Toutes"}
          </button>
        ))}
      </div>

      <div className="space-y-3">
        {filtered.map(a => (
          <div key={a.id} className="bg-gc-card border border-gc-border rounded-lg p-4" style={{ borderLeftWidth: 3, borderLeftColor: COLORS[a.type] }}>
            <div className="flex justify-between items-start">
              <div>
                <div className="flex gap-2 mb-2 flex-wrap">
                  <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold" style={{ background: COLORS[a.type] + "20", color: COLORS[a.type], border: "1px solid " + COLORS[a.type] + "30" }}>
                    {LABELS[a.type]}
                  </span>
                  {a.track !== "all" && (
                    <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold" style={{ background: (TRACK_COLORS[a.track]||"#00ff88") + "20", color: TRACK_COLORS[a.track]||"#00ff88" }}>
                      {a.track.toUpperCase()}
                    </span>
                  )}
                  {a.level !== "all" && <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-gc-neon/15 text-gc-neon">{a.level}</span>}
                </div>
                <div className="text-sm font-bold text-gc-text">{a.title}</div>
                <div className="text-xs text-gc-muted mt-1">{a.description}</div>
              </div>
              <div className="text-right shrink-0 ml-4">
                <div className="text-xs font-mono text-gc-cyan">{new Date(a.date).toLocaleDateString("fr-FR")}</div>
                <div className="text-[10px] text-gc-muted mt-1">{a.duration}</div>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
