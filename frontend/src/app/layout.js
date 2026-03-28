import "@/styles/globals.css";

export const metadata = { title: "GameCloud Intranet", description: "Plateforme GameCloud" };

export default function RootLayout({ children }) {
  return (
    <html lang="fr">
      <body>{children}</body>
    </html>
  );
}
