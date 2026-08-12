import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [svelte(), tailwindcss()],

  // Die Tests hängen die Bausteine an ein echtes Dokument, statt sie
  // serverseitig zu Text zu rendern. Der Unterschied ist nicht akademisch:
  // Eine weiße Seite entsteht beim START im Browser — durch Module auf
  // oberster Ebene, Effekte, Zugriffe auf `document`. Serverseitiges
  // Rendern sieht davon nichts und bleibt grün.
  test: {
    environment: "happy-dom",
  },
  resolve: process.env.VITEST ? { conditions: ["browser"] } : {},
});
