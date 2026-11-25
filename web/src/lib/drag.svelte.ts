import { findUrls, normalizeUrl } from "./links";

// Verschieben von Bookmarks und Ordnern per Ziehen, für Maus und Touch.
//
// Maus: Ziehen beginnt nach ein paar Pixeln Bewegung.
// Touch: Ziehen beginnt nach langem Drücken; kurzes Wischen scrollt normal.
// Ablageziele sind Elemente mit `data-drop` ("none" = Unsortiert, sonst Ordner-ID).

export type DragItem =
  | { kind: "bookmark"; id: number; label: string }
  | { kind: "folder"; id: number; label: string };

/** Ablageziel: Ordner-ID oder `null` für „Unsortiert“. */
export type DropTarget = number | null;

const LONG_PRESS_MS = 450;
const MOUSE_THRESHOLD = 6;
const TOUCH_TOLERANCE = 10;

class DragState {
  /** Gerade gezogenes Element. */
  item = $state<DragItem | null>(null);
  x = $state(0);
  y = $state(0);
  /** Wert von `data-drop` unter dem Zeiger (auch beim Ziehen von Links von außen). */
  target = $state<string | null>(null);

  /** Wird beim Ablegen aufgerufen (von der Seite gesetzt). */
  ondrop: ((item: DragItem, target: DropTarget) => void) | null = null;
  /** Wird beim Start aufgerufen, z. B. um am Handy das Menü zu öffnen. */
  onstart: ((item: DragItem) => void) | null = null;
}

export const drag = new DragState();

export function parseTarget(value: string | null | undefined): DropTarget | undefined {
  if (value === undefined || value === null) return undefined;
  if (value === "none") return null;
  const id = Number(value);
  return Number.isInteger(id) && id > 0 ? id : undefined;
}

export function targetAt(x: number, y: number): string | null {
  const element = document.elementFromPoint(x, y);
  const target = element?.closest<HTMLElement>("[data-drop]");
  return target?.dataset.drop ?? null;
}

/** Svelte-Action: macht ein Element ziehbar. */
export function draggable(node: HTMLElement, initial: DragItem) {
  let item = initial;
  let pointerId: number | null = null;
  let pointerType = "";
  let startX = 0;
  let startY = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  /** Mit Escape abgebrochen; der Finger bzw. die Maustaste ist noch unten. */
  let cancelled = false;

  function down(event: PointerEvent) {
    if (event.button !== 0 || drag.item || pointerId !== null) return;
    if ((event.target as Element).closest("button, input, select, textarea")) return;
    pointerId = event.pointerId;
    pointerType = event.pointerType;
    startX = event.clientX;
    startY = event.clientY;
    if (pointerType !== "mouse") timer = setTimeout(() => begin(startX, startY), LONG_PRESS_MS);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
  }

  function begin(x: number, y: number) {
    clearTimeout(timer);
    drag.item = item;
    drag.x = x;
    drag.y = y;
    drag.target = null;
    document.documentElement.classList.add("dragging");
    if (pointerType !== "mouse") navigator.vibrate?.(15);
    window.addEventListener("keydown", keydown, true);
    drag.onstart?.(item);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || !drag.item) return;
    event.preventDefault();
    event.stopPropagation();
    finish();
    cancelled = true;
  }

  function move(event: PointerEvent) {
    if (event.pointerId !== pointerId || cancelled) return;
    if (drag.item) {
      drag.x = event.clientX;
      drag.y = event.clientY;
      drag.target = targetAt(event.clientX, event.clientY);
      return;
    }
    const distance = Math.hypot(event.clientX - startX, event.clientY - startY);
    if (pointerType === "mouse") {
      if (distance > MOUSE_THRESHOLD) begin(event.clientX, event.clientY);
    } else if (distance > TOUCH_TOLERANCE) {
      // Finger bewegt sich vor Ablauf der Wartezeit: normales Scrollen.
      cleanup();
    }
  }

  function up(event: PointerEvent) {
    if (event.pointerId !== pointerId) return;
    if (cancelled) {
      suppressClick();
    } else if (drag.item) {
      const target = parseTarget(targetAt(event.clientX, event.clientY));
      const dragged = drag.item;
      finish();
      suppressClick();
      if (target !== undefined) drag.ondrop?.(dragged, target);
    }
    cleanup();
  }

  function cancel(event: PointerEvent) {
    if (event.pointerId !== pointerId) return;
    if (drag.item) finish();
    cleanup();
  }

  function finish() {
    drag.item = null;
    drag.target = null;
    document.documentElement.classList.remove("dragging");
  }

  function cleanup() {
    clearTimeout(timer);
    pointerId = null;
    cancelled = false;
    window.removeEventListener("keydown", keydown, true);
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", cancel);
  }

  // Während des Ziehens nicht scrollen (muss schon beim Berühren registriert sein).
  function touchmove(event: TouchEvent) {
    if (drag.item && pointerId !== null) event.preventDefault();
  }

  // Langes Drücken öffnet sonst das Kontextmenü des Links.
  function contextmenu(event: Event) {
    if (pointerId !== null && pointerType !== "mouse") event.preventDefault();
  }

  node.addEventListener("pointerdown", down);
  node.addEventListener("touchmove", touchmove, { passive: false });
  node.addEventListener("contextmenu", contextmenu);
  node.addEventListener("dragstart", preventNativeDrag);

  return {
    update(next: DragItem) {
      item = next;
    },
    destroy() {
      if (pointerId !== null && drag.item) finish();
      cleanup();
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("touchmove", touchmove);
      node.removeEventListener("contextmenu", contextmenu);
      node.removeEventListener("dragstart", preventNativeDrag);
    },
  };
}

// Das native Ziehen von Links und Bildern würde mit dem eigenen Ziehen kollidieren.
function preventNativeDrag(event: DragEvent) {
  event.preventDefault();
}

/** Der Klick nach dem Loslassen soll den Link nicht öffnen. */
function suppressClick() {
  const block = (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
  };
  window.addEventListener("click", block, { capture: true, once: true });
  setTimeout(() => window.removeEventListener("click", block, { capture: true }), 400);
}

export interface DroppedLink {
  url: string;
  title?: string;
}

const MAX_DROPPED = 100;

/** Enthält das Ziehen von außen einen Link oder Text? */
export function hasLink(data: DataTransfer | null): boolean {
  if (!data) return false;
  const types = Array.from(data.types);
  return ["text/uri-list", "text/plain", "text/x-moz-url", "text/html"].some((type) =>
    types.includes(type),
  );
}

/** Links aus einem Ablegen von außen lesen (Browser-Tab, Lesezeichen, Text). */
export function droppedLinks(data: DataTransfer): DroppedLink[] {
  const links: DroppedLink[] = [];

  // Firefox: abwechselnd URL und Titel
  const moz = data.getData("text/x-moz-url");
  if (moz) {
    const lines = moz.split(/\r?\n/);
    for (let i = 0; i < lines.length; i += 2) {
      const url = lines[i];
      if (url) links.push({ url, title: lines[i + 1] });
    }
  }

  if (links.length === 0) {
    const uris = data
      .getData("text/uri-list")
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line !== "" && !line.startsWith("#"));
    const title = uris.length === 1 ? titleFromHtml(data.getData("text/html")) : undefined;
    for (const url of uris) links.push({ url, title });
  }

  if (links.length === 0) {
    const html = data.getData("text/html");
    if (html) {
      const doc = new DOMParser().parseFromString(html, "text/html");
      for (const anchor of Array.from(doc.querySelectorAll("a[href]")).slice(0, MAX_DROPPED)) {
        const href = anchor.getAttribute("href") ?? "";
        const text = anchor.textContent?.replace(/s+/g, " ").trim();
        links.push({ url: href, title: text ? text.slice(0, 300) : undefined });
      }
    }
  }

  if (links.length === 0) {
    const text = data.getData("text/plain");
    for (const url of findUrls(text)) links.push({ url });
  }

  const seen = new Set<string>();
  const result: DroppedLink[] = [];
  for (const link of links) {
    const url = normalizeUrl(link.url);
    if (!url || seen.has(url)) continue;
    seen.add(url);
    const title = link.title?.trim();
    result.push({ url, title: title && title !== link.url.trim() ? title : undefined });
    if (result.length >= MAX_DROPPED) break;
  }
  return result;
}


// Text des ersten Links. DOMParser führt nichts aus; das Ergebnis wird nur als Text verwendet.
function titleFromHtml(html: string): string | undefined {
  if (!html) return undefined;
  const doc = new DOMParser().parseFromString(html, "text/html");
  const text = doc.querySelector("a")?.textContent?.replace(/\s+/g, " ").trim();
  return text ? text.slice(0, 300) : undefined;
}
