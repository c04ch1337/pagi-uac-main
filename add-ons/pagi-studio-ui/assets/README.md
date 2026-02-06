# Studio UI assets (bare-metal, no external deps)

- **ui_config.json** — Studio settings (window size, default KB slot, theme). Bundled at compile time via `include_str!("../assets/ui_config.json")` in `src/config.rs`; a local file overrides the default if present.
- **studio-interface/** — Google Studio style web UI (React/Vite). Used by `pagi-studio-ui-server`: static files are served from here (or from `studio-interface/dist` after `npm run build`). `index.css` and other assets live under this folder; the server serves them at runtime.
- **CSS / Icons / Fonts** — For Rust-only assets, use `include_str!` or `include_bytes!` with paths relative to the source file so the binary has no runtime file or CDN dependency.
