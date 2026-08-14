import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [svelte(), tailwindcss()],

  /**
   * Fester Port, und zwar streng.
   *
   * `crates/cabrik-fenster/tauri.conf.json` trägt `devUrl` fest ein. Ist
   * 5173 belegt, weicht Vite von sich aus auf 5174 aus, sagt es beiläufig
   * in einer Zeile — und das Fenster lädt dann von einem Port, an dem
   * nichts horcht. Es zeigt „Seite nicht erreichbar“ und nennt keinen
   * Grund.
   *
   * `strictPort` macht daraus einen Fehlschlag beim Start des Servers, wo
   * er hingehört: an die Stelle, an der jemand zusieht.
   */
  server: {
    port: 5173,
    strictPort: true,
  },

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
