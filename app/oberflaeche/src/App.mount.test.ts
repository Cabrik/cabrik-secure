/**
 * Hängt die Anwendung wirklich an ein Dokument.
 *
 * Die übrigen Tests stellen einzelne Bildschirme serverseitig dar. Was sie
 * NICHT prüfen, ist der Start im Browser: Module auf oberster Ebene,
 * Effekte, Zugriffe auf `document`. Genau dort kann eine weiße Seite
 * entstehen, während jeder andere Test grün bleibt.
 */
// @vitest-environment happy-dom

import { expect, it } from "vitest";
import { mount, unmount } from "svelte";
import App from "./App.svelte";

it("die Anwendung startet und stellt etwas dar", () => {
  document.body.innerHTML = '<div id="app"></div>';
  const ziel = document.getElementById("app")!;

  const a = mount(App, { target: ziel });
  expect(ziel.textContent).toContain("Cabrik");
  expect(ziel.textContent!.length).toBeGreaterThan(200);
  unmount(a);
});
