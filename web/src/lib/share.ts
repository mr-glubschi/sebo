// Android „Teilen“ (Web Share Target): Die App wird mit /share?url=…&text=…&title=… geöffnet.
// Manche Apps schicken den Link in `text` oder `title` statt in `url`.

const URL_PATTERN = /https?:\/\/[^\s<>"']+/;

export interface Shared {
  url: string;
  title?: string;
}

/** Liest einen geteilten Link aus der Adresse und räumt die Adresse auf. */
export function readShare(): Shared | null {
  if (location.pathname !== "/share") return null;
  const params = new URLSearchParams(location.search);
  history.replaceState(null, "", "/");

  const fields = ["url", "text", "title"].map((name) => params.get(name)?.trim() ?? "");
  let url = "";
  for (const field of fields) {
    const match = URL_PATTERN.exec(field);
    if (match) {
      url = match[0];
      break;
    }
  }
  if (!url) return null;

  // Titel: das title-Feld, falls es nicht selbst nur der Link ist.
  const title = fields[2] && !URL_PATTERN.test(fields[2]) ? fields[2].slice(0, 300) : undefined;
  return { url, title };
}
