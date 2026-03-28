"use client";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Sidebar from "@/components/Sidebar";

export default function DashboardLayout({ children }) {
  const [ok, setOk] = useState(false);
  const router = useRouter();

  useEffect(() => {
    if (!localStorage.getItem("gc_token")) {
      router.replace("/login");
    } else {
      setOk(true);
    }
  }, [router]);

  if (!ok) return <div className="min-h-screen bg-gc-bg flex items-center justify-center"><span className="text-gc-neon font-mono glow-green">Chargement...</span></div>;

  return (
    <div className="min-h-screen bg-gc-bg">
      <Sidebar />
      <main className="ml-56 p-6 min-h-screen">{children}</main>
    </div>
  );
}
