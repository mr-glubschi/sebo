// Mehrfachauswahl: entweder Bookmarks (Hauptbereich) oder Ordner (Seitenleiste).

export type SelectionKind = "bookmark" | "folder";

class Selection {
  kind = $state<SelectionKind | null>(null);
  /** Wird bei jeder Änderung ersetzt, damit Svelte sie bemerkt. */
  ids = $state<ReadonlySet<number>>(new Set());
  /** Zuletzt angeklickter Eintrag, Ausgangspunkt für Umschalt-Klick. */
  anchor: number | null = null;

  get count() {
    return this.ids.size;
  }

  is(kind: SelectionKind) {
    return this.kind === kind;
  }

  start(kind: SelectionKind, id?: number) {
    this.kind = kind;
    this.ids = new Set(id === undefined ? [] : [id]);
    this.anchor = id ?? null;
  }

  toggle(id: number) {
    const next = new Set(this.ids);
    if (!next.delete(id)) next.add(id);
    this.ids = next;
    this.anchor = id;
  }

  add(ids: Iterable<number>) {
    this.ids = new Set([...this.ids, ...ids]);
  }

  set(ids: Iterable<number>) {
    this.ids = new Set(ids);
  }

  /** Nicht mehr vorhandene Einträge entfernen (z. B. in einem anderen Tab gelöscht). */
  keep(existing: (id: number) => boolean) {
    if ([...this.ids].every(existing)) return;
    this.ids = new Set([...this.ids].filter(existing));
  }

  clear() {
    this.kind = null;
    this.ids = new Set();
    this.anchor = null;
  }
}

export const selection = new Selection();
