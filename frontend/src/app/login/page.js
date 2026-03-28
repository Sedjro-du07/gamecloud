"use client";
import { useState } from "react";
import { useRouter } from "next/navigation";
import { login } from "@/lib/api";

export default function LoginPage() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  async function handleSubmit(e) {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await login(email, password);
      router.push("/dashboard");
    } catch (err) {
      setError(err.message || "Identifiants incorrects");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen bg-gc-bg flex items-center justify-center relative overflow-hidden">
      {/* Background dots */}
      <div className="absolute inset-0 opacity-5">
        {Array.from({ length: 50 }).map((_, i) => (
          <div key={i} className="absolute w-1 h-1 rounded-full bg-gc-neon"
            style={{ left: Math.random()*100+"%", top: Math.random()*100+"%", opacity: 0.3+Math.random()*0.7 }} />
        ))}
      </div>

      <div className="relative w-full max-w-md p-8">
        {/* Logo */}
        <div className="text-center mb-10">
          <div className="text-5xl mb-3">🎮</div>
          <h1 className="text-3xl font-bold text-gc-neon glow-green font-mono tracking-wider">GAMECLOUD</h1>
          <p className="text-gc-muted text-sm mt-2 font-mono tracking-widest">INTRANET PLATFORM</p>
        </div>

        {/* Login card */}
        <div className="bg-gc-card border border-gc-border rounded-xl p-6 neon-border">
          <h2 className="text-lg font-bold text-gc-text mb-6 font-display">Connexion</h2>

          {error && (
            <div className="mb-4 p-3 rounded-lg bg-red-500/10 border border-red-500/30 text-red-400 text-sm font-mono">
              {error}
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-xs text-gc-muted font-mono mb-1.5 tracking-wider">EMAIL</label>
              <input type="email" value={email} onChange={e => setEmail(e.target.value)} required
                className="w-full px-4 py-3 bg-gc-bg border border-gc-border rounded-lg text-gc-text font-mono text-sm focus:border-gc-neon focus:outline-none transition-colors"
                placeholder="email@gamecloud.bj" />
            </div>
            <div>
              <label className="block text-xs text-gc-muted font-mono mb-1.5 tracking-wider">MOT DE PASSE</label>
              <input type="password" value={password} onChange={e => setPassword(e.target.value)} required
                className="w-full px-4 py-3 bg-gc-bg border border-gc-border rounded-lg text-gc-text font-mono text-sm focus:border-gc-neon focus:outline-none transition-colors"
                placeholder="••••••••" />
            </div>
            <button type="submit" disabled={loading}
              className="w-full py-3 rounded-lg font-mono font-bold text-sm tracking-wider transition-all disabled:opacity-50 bg-gc-neon/15 border border-gc-neon/40 text-gc-neon hover:bg-gc-neon/25 hover:shadow-[0_0_20px_rgba(0,255,136,0.2)]">
              {loading ? "CONNEXION..." : "SE CONNECTER"}
            </button>
          </form>
        </div>

        <p className="text-center text-gc-dim text-xs font-mono mt-6">
          GameCloud — Epitech Benin
        </p>
      </div>
    </div>
  );
}
