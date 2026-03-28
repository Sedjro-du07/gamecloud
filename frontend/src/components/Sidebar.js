"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { logout, getUser, getRoleLevel } from "@/lib/api";

const ROLE_COLORS = { PRESIDENT: "#ffd700", VP: "#ff00aa", ADMIN: "#00d4ff", MEMBER: "#00ff88" };
const ROLE_ICONS = { PRESIDENT: "👑", VP: "⚡", ADMIN: "🛡️", MEMBER: "🎮" };
const TRACK_COLORS = { ENGINEERING: "#00d4ff", DESIGN: "#ff00aa", AUDIO: "#ff8800", VISUALS: "#8844ff" };

export default function Sidebar() {
  const path = usePathname();
  const user = getUser();
  if (!user) return null;

  const level = getRoleLevel(user.role);
  const pages = [
    { href: "/dashboard", icon: "🏠", label: "Dashboard" },
    { href: "/skills", icon: "🧬", label: "Skills Tree" },
    { href: "/activities", icon: "📅", label: "Activites" },
    { href: "/members", icon: "👥", label: "Membres" },
  ];
  if (level >= 3) pages.push({ href: "/validation", icon: "✅", label: "Validation" });
  if (level >= 2) pages.push({ href: "/admin", icon: "⚙️", label: "Admin" });

  return (
    <aside className="fixed left-0 top-0 h-screen w-56 bg-gc-panel border-r border-gc-border flex flex-col z-50">
      <div className="p-4 border-b border-gc-border">
        <div className="flex items-center gap-2">
          <span className="text-xl">🎮</span>
          <span className="text-lg font-bold text-gc-neon glow-green font-mono">GAMECLOUD</span>
        </div>
        <div className="text-[10px] text-gc-dim font-mono tracking-widest mt-1">INTRANET v1.0</div>
      </div>

      <div className="p-3 border-b border-gc-border">
        <div className="flex items-center gap-2">
          <div className="w-9 h-9 rounded-lg flex items-center justify-center text-lg"
            style={{ background: ROLE_COLORS[user.role] + "20", border: "1px solid " + ROLE_COLORS[user.role] + "40" }}>
            {ROLE_ICONS[user.role]}
          </div>
          <div>
            <div className="text-sm font-bold text-gc-text">{user.name}</div>
            <span className="text-[10px] font-mono px-2 py-0.5 rounded"
              style={{ background: ROLE_COLORS[user.role] + "20", color: ROLE_COLORS[user.role], border: "1px solid " + ROLE_COLORS[user.role] + "30" }}>
              {user.role}
            </span>
          </div>
        </div>
      </div>

      <nav className="flex-1 p-2 space-y-1">
        {pages.map(p => {
          const active = path === p.href;
          return (
            <Link key={p.href} href={p.href}
              className={`flex items-center gap-3 px-3 py-2.5 rounded-md text-sm font-semibold transition-all ${active ? "bg-gc-neon/10 text-gc-neon border border-gc-neon/30" : "text-gc-muted hover:text-gc-text hover:bg-gc-border/30 border border-transparent"}`}>
              <span>{p.icon}</span> {p.label}
            </Link>
          );
        })}
      </nav>

      <div className="p-3 border-t border-gc-border space-y-1">
        <div className="text-[10px] text-gc-dim font-mono">TRACK: <span style={{ color: TRACK_COLORS[user.track] }}>{user.track}</span></div>
        <div className="text-[10px] text-gc-dim font-mono">LEVEL: {user.level}</div>
        <div className="text-[10px] text-gc-dim font-mono">XP: {user.xp}</div>
        <button onClick={logout}
          className="w-full mt-2 px-3 py-1.5 rounded text-xs font-mono font-bold text-red-400 border border-red-400/30 hover:bg-red-400/10 transition-all">
          DECONNEXION
        </button>
      </div>
    </aside>
  );
}
