// ==================== KaTeX Initialization ====================
// Loads KaTeX and exposes it on window.__ecall_katex for the renderer to use.
// KaTeX CSS is imported at each entry point (main-*.ts), not here.

import katex from "katex";

let initialized = false;

export function initKatex(): void {
  if (initialized || typeof window === "undefined") return;
  initialized = true;
  (window as any).__ecall_katex = katex;
}
