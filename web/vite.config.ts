import { defineConfig } from "vite";

// 前端构建产物直接输出到 Tauri 的 frontendDist 目录
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "../desktop/dist",
    emptyOutDir: true,
    target: "chrome110",
  },
});
