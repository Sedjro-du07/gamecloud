/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{js,jsx}"],
  theme: {
    extend: {
      colors: {
        gc: {
          bg: "#05050a",
          card: "#0c0c14",
          panel: "#0f0f1a",
          border: "#1a1a30",
          neon: "#00ff88",
          cyan: "#00d4ff",
          magenta: "#ff00aa",
          orange: "#ff8800",
          purple: "#8844ff",
          text: "#e8e8f0",
          muted: "#6a6a88",
          dim: "#3a3a55",
        },
      },
      fontFamily: {
        display: ["Rajdhani", "sans-serif"],
        mono: ["Chakra Petch", "monospace"],
      },
    },
  },
  plugins: [],
};
