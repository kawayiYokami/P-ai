/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{vue,ts,js}"],
  theme: {
    extend: {},
  },
  plugins: [
    require("daisyui", {
      exclude: ["scrollbar"],
    }),
  ],
  daisyui: {
    themes: ["light", "dracula"],
  },
};
