"use client";
import { useEffect } from "react";
import { useRouter } from "next/navigation";

export default function Home() {
  const router = useRouter();
  useEffect(() => {
    const token = localStorage.getItem("gc_token");
    router.replace(token ? "/dashboard" : "/login");
  }, [router]);
  return (
    <div className="min-h-screen bg-gc-bg flex items-center justify-center">
      <div className="text-gc-neon font-mono text-sm glow-green">Chargement...</div>
    </div>
  );
}
