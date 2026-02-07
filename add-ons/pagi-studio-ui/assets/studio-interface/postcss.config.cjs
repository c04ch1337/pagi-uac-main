// Explicit PostCSS config so Vite does NOT load `tailwindcss` as a PostCSS plugin.
// Tailwind v4 is handled by @tailwindcss/vite only. Using tailwindcss here causes:
// "It looks like you're trying to use tailwindcss directly as a PostCSS plugin..."
module.exports = {
  plugins: {
    autoprefixer: {},
  },
};
