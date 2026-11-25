// Unscharfe Suche über Titel und Domain (uFuzzy), komplett im Browser.

import uFuzzy from "@leeoniya/ufuzzy";
import type { Bookmark } from "./bindings/Bookmark";
import { host } from "./library";

/** Titel in Abschnitte zerlegt, `match` = Treffer (zum Hervorheben, nur als Text). */
export type Part = { text: string; match: boolean };

export interface Hit {
  bookmark: Bookmark;
  title: Part[];
}

const MAX_RESULTS = 200;

// Unicode-Einstellungen laut uFuzzy-Doku, damit Umlaute und andere Schriften Wortteile sind.
const fuzzy = new uFuzzy({
  intraMode: 1,
  intraIns: 1,
  unicode: true,
  interSplit: String.raw`[^\p{L}\d']+`,
  intraSplit: String.raw`\p{Ll}\p{Lu}`,
  intraBound: String.raw`\p{Ll}\p{Lu}`,
  intraChars: String.raw`[\p{L}\d']`,
  intraContr: String.raw`'\p{L}{1,2}\b`,
});

/** „Ärger“ findet auch „arger“ und umgekehrt (gleiche Länge, Treffer bleiben passend). */
function fold(text: string): string {
  return uFuzzy.latinize(text);
}

export class SearchIndex {
  private readonly haystack: string[];

  constructor(private readonly bookmarks: Bookmark[]) {
    // Titel und Domain durch einen Zeilenumbruch getrennt, damit Treffer
    // nicht über die Grenze hinweg zusammengesetzt werden.
    this.haystack = bookmarks.map((b) => fold(`${b.title}\n${host(b.url)}`));
  }

  search(query: string): Hit[] {
    const needle = fold(query.trim());
    if (needle === "") return [];
    // outOfOrder: Suchwörter in beliebiger Reihenfolge.
    const [indexes, info, order] = fuzzy.search(this.haystack, needle, 1, 1000);
    if (!indexes) return [];

    const hits: Hit[] = [];
    if (info && order) {
      for (const position of order.slice(0, MAX_RESULTS)) {
        const index = info.idx[position];
        const bookmark = index === undefined ? undefined : this.bookmarks[index];
        if (!bookmark) continue;
        hits.push({ bookmark, title: highlight(bookmark.title, info.ranges[position] ?? []) });
      }
    } else {
      // Zu viele Treffer für die Rangfolge: in Originalreihenfolge.
      for (const index of indexes.slice(0, MAX_RESULTS)) {
        const bookmark = this.bookmarks[index];
        if (bookmark) hits.push({ bookmark, title: plain(bookmark.title) });
      }
    }
    return hits;
  }
}

function plain(text: string): Part[] {
  return [{ text, match: false }];
}

/** Zerlegt den Titel anhand der Trefferbereiche (Paare aus Start und Ende). */
function highlight(title: string, ranges: number[]): Part[] {
  const parts: Part[] = [];
  let position = 0;
  for (let i = 0; i + 1 < ranges.length; i += 2) {
    const start = ranges[i] ?? 0;
    const end = Math.min(ranges[i + 1] ?? 0, title.length);
    if (start >= title.length) break;
    if (start > position) parts.push({ text: title.slice(position, start), match: false });
    if (end > start) parts.push({ text: title.slice(start, end), match: true });
    position = Math.max(position, end);
  }
  if (position < title.length) parts.push({ text: title.slice(position), match: false });
  return parts;
}
