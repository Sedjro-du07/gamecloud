const API = "http://localhost:3001/api";

export async function api(path, options = {}) {
  let token = null;
  if (typeof window !== "undefined") token = localStorage.getItem("gc_token");
  const res = await fetch(API + path, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: "Bearer " + token } : {}),
      ...options.headers,
    },
  });
  if (res.status === 401 && typeof window !== "undefined") {
    localStorage.removeItem("gc_token");
    localStorage.removeItem("gc_user");
    window.location.href = "/login";
    throw new Error("Session expiree");
  }
  const data = await res.json();
  if (!res.ok) throw new Error(data.error || "Erreur");
  return data;
}

export async function login(email, password) {
  const data = await api("/auth/login", {
    method: "POST",
    body: JSON.stringify({ email, password }),
  });
  localStorage.setItem("gc_token", data.token);
  localStorage.setItem("gc_refresh", data.refreshToken);
  localStorage.setItem("gc_user", JSON.stringify(data.user));
  return data.user;
}

export function logout() {
  localStorage.removeItem("gc_token");
  localStorage.removeItem("gc_refresh");
  localStorage.removeItem("gc_user");
  window.location.href = "/login";
}

export function getUser() {
  if (typeof window === "undefined") return null;
  const u = localStorage.getItem("gc_user");
  return u ? JSON.parse(u) : null;
}

export function getRoleLevel(role) {
  return { MEMBER: 1, ADMIN: 2, VP: 3, PRESIDENT: 4 }[role] || 0;
}
