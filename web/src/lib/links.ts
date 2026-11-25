// Links in freiem Text finden (Ablegen von Text, Teilen, Einfügen).

const URL_PATTERN = /https?:\/\/[^\s<>"'`]+/g;
const TRAILING = /[.,;:!?)\]}'"»«“”‘’]$/;

function count(text: string, char: string): number {
  return text.split(char).length - 1;
}

/**
 * Alle http(s)-Adressen im Text. Satzzeichen am Ende („Siehe https://a.de/.“)
 * gehören zum Satz, nicht zur Adresse. Schließende Klammern bleiben, wenn die
 * Adresse selbst eine öffnende enthält (z. B. Wikipedia-Artikel).
 */
export function findUrls(text: string): string[] {
  const result: string[] = [];
  for (const match of text.matchAll(URL_PATTERN)) {
    let url = match[0];
    while (TRAILING.test(url)) {
      const last = url.charAt(url.length - 1);
      if (last === ")" && count(url, "(") >= count(url, ")")) break;
      if (last === "]" && count(url, "[") >= count(url, "]")) break;
      url = url.slice(0, -1);
    }
    result.push(url);
  }
  return result;
}

/** Nur http(s); gibt die normalisierte Form zurück. */
export function normalizeUrl(raw: string): string | null {
  try {
    const url = new URL(raw.trim());
    return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}
