// Feste Farbpalette für Ordner-Symbole. In der DB steht nur der Schlüssel; die
// Farbwerte stehen als `--folder-<schlüssel>` in app.css (je für hell und dunkel).
// Jede Farbe hat gegen alle Hintergründe der Seitenleiste (auch ausgewählt und
// beim Ziehen) mindestens 3:1 Kontrast. Ohne Farbe: Akzentfarbe.

export const FOLDER_COLORS: Record<string, string> = {
  red: "Rot",
  orange: "Orange",
  yellow: "Gelb",
  green: "Grün",
  teal: "Türkis",
  blue: "Blau",
  indigo: "Indigo",
  purple: "Lila",
  pink: "Pink",
  brown: "Braun",
  gray: "Grau",
};

export const FOLDER_COLOR_NAMES = Object.keys(FOLDER_COLORS);

/** CSS-Farbe für einen gespeicherten Schlüssel; unbekannt oder leer = Akzentfarbe. */
export function folderColor(name: string | null | undefined): string {
  return name && Object.hasOwn(FOLDER_COLORS, name) ? `var(--folder-${name})` : "var(--accent)";
}
