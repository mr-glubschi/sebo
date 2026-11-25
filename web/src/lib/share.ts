// Android „Teilen“ (Web Share Target): Die App wird mit /share?url=…&text=…&title=… geöffnet.
// Manche Apps schicken den Link in `text` oder `title` statt in `url`.

import { findUrls } from "./links";

export interface Shared {
  /** Fehlt, wenn im geteilten Inhalt kein Link war. */
  url?: string;
  title?: string;
}

/** Liest einen geteilten Link aus der Adresse und räumt die Adresse auf. */
export function readShare(): Shared | null {
  if (location.pathname !== "/share") return null;
  const params = new URLSearchParams(location.search);
  history.replaceState(null, "", "/");

  const fields = ["url", "text", "title"].map((name) => params.get(name)?.trim() ?? "");
  const url = fields.map((field) => findUrls(field)[0]).find(Boolean);
  if (!url) return {};

  // Titel: das title-Feld, falls es nicht selbst einen Link enthält.
  const title = fields[2] && findUrls(fields[2]).length === 0 ? fields[2].slice(0, 300) : undefined;
  return { url, title };
}
