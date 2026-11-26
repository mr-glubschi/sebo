<script lang="ts">
  import { tick, untrack } from "svelte";
  import { api, errorText } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import type { Folder } from "./bindings/Folder";
  import type { Library } from "./bindings/Library";
  import BookmarkDialog from "./BookmarkDialog.svelte";
  import BookmarkView from "./BookmarkView.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import { menu, type MenuItem } from "./menu.svelte";
  import DeleteFoldersDialog from "./DeleteFoldersDialog.svelte";
  import {
    drag,
    droppedLinks,
    hasLink,
    parseTarget,
    targetAt,
    type DragItem,
    type DropTarget,
    type DroppedLink,
  } from "./drag.svelte";
  import FolderDialog from "./FolderDialog.svelte";
  import FolderIcon from "./FolderIcon.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import {
    buildTree,
    folderPath,
    loadPref,
    parseRoute,
    routePath,
    sameRoute,
    savePref,
    SORT_MODES,
    sortBookmarks,
    subtree,
    VIEW_MODES,
    type Route,
    type SortMode,
    type ViewMode,
  } from "./library";
  import MergeDialog from "./MergeDialog.svelte";
  import MoveDialog from "./MoveDialog.svelte";
  import { SearchIndex, type Part } from "./search";
  import { selection } from "./selection.svelte";
  import { readShare } from "./share";
  import Sidebar from "./Sidebar.svelte";
  import { toasts } from "./toast.svelte";
  import Toasts from "./Toasts.svelte";

  let { onlogout }: { onlogout: () => void } = $props();

  type Dialog =
    | { kind: "add"; url?: string; title?: string }
    | { kind: "edit"; bookmark: Bookmark }
    | { kind: "newFolder"; parentId: number | null }
    | { kind: "editFolder"; folder: Folder }
    /** `bulk`: aus der Mehrfachauswahl, danach wird sie beendet. */
    | { kind: "move"; target: "bookmark" | "folder"; ids: number[]; bulk: boolean }
    | { kind: "deleteBookmarks"; ids: number[]; bulk: boolean }
    | { kind: "deleteFolders"; ids: number[]; bulk: boolean }
    | { kind: "merge"; ids: number[] };

  // Über „Teilen“ (Android) geöffnet: Link in den Dialog übernehmen.
  const shared = readShare();

  let library = $state<Library | null>(null);
  let loadError = $state("");
  let route = $state<Route>(parseRoute(location.pathname));
  let view = $state<ViewMode>(loadPref("view", VIEW_MODES, "cards"));
  let sort = $state<SortMode>(loadPref("sort", SORT_MODES, "new"));
  let menuOpen = $state(false);
  let dialog = $state<Dialog | null>(
    shared?.url ? { kind: "add", url: shared.url, title: shared.title } : null,
  );
  if (shared && !shared.url) {
    queueMicrotask(() => toasts.show("Im geteilten Inhalt wurde kein Link gefunden.", true));
  }
  // Unbekannte Adresse (z. B. /irgendwas): auf die passende Adresse korrigieren.
  if (!shared && routePath(parseRoute(location.pathname)) !== location.pathname) {
    history.replaceState(null, "", routePath(parseRoute(location.pathname)));
  }
  let query = $state("");
  let searchInput = $state<HTMLInputElement>();
  /** Gerade wird ein Link von außen über die Seite gezogen. */
  let external = $state(false);

  const tree = $derived(buildTree(library?.folders ?? []));
  const bookmarks = $derived(library?.bookmarks ?? []);
  const index = $derived(new SearchIndex(bookmarks));
  const searching = $derived(query.trim() !== "");
  const result = $derived(searching ? index.search(query) : { hits: [], total: 0 });
  const hits = $derived(result.hits);

  const counts = $derived.by(() => {
    const result = new Map<number, number>();
    for (const b of bookmarks) {
      if (b.folder_id !== null) result.set(b.folder_id, (result.get(b.folder_id) ?? 0) + 1);
    }
    return result;
  });
  const unsortedCount = $derived(bookmarks.filter((b) => b.folder_id === null).length);
  const favoritesCount = $derived(bookmarks.filter((b) => b.favorite).length);

  const current = $derived(route.kind === "folder" ? tree.byId.get(route.id) : undefined);
  const breadcrumbs = $derived(
    current && !searching ? folderPath(tree, current.id).slice(0, -1) : [],
  );
  const heading = $derived(
    searching
      ? "Suche"
      : route.kind === "favorites"
        ? "Favoriten"
        : route.kind === "all"
          ? "Alle Bookmarks"
          : route.kind === "unsorted"
            ? "Unsortiert"
            : (current?.name ?? ""),
  );
  const subfolders = $derived(
    current && !searching ? (tree.children.get(current.id) ?? []) : [],
  );
  const visible = $derived(
    searching
      ? hits.map((hit) => hit.bookmark)
      : sortBookmarks(
          bookmarks.filter((b) =>
            route.kind === "all"
              ? true
              : route.kind === "favorites"
                ? b.favorite
                : route.kind === "unsorted"
                ? b.folder_id === null
                : b.folder_id === route.id,
          ),
          sort,
        ),
  );
  const highlights = $derived(
    searching ? new Map<number, Part[]>(hits.map((hit) => [hit.bookmark.id, hit.title])) : undefined,
  );
  const showFolders = $derived(searching || route.kind === "all" || route.kind === "favorites");
  const currentFolderId = $derived(route.kind === "folder" ? route.id : null);

  function folderName(id: number | null): string {
    return id === null ? "Unsortiert" : (tree.byId.get(id)?.name ?? "?");
  }

  function targetName(target: DropTarget): string {
    if (target === "unsorted") return "Unsortiert";
    if (target === "root") return "oberste Ebene";
    if (target === "favorites") return "Favoriten";
    if (typeof target === "object") return folderName(target.id);
    return folderName(target);
  }

  $effect(() => {
    document.title = heading ? `${heading} – SEBO` : "SEBO";
  });

  /** ETag des angezeigten Stands; nur beim Nachfragen mitgeschickt. */
  let etag: string | undefined;

  async function reload(onlyIfChanged = false) {
    try {
      const result = await api.library(onlyIfChanged ? etag : undefined);
      if (result) {
        library = result.library;
        etag = result.etag;
      }
      loadError = "";
    } catch (e) {
      loadError = errorText(e);
    }
  }

  // Solange Titel und Bilder im Hintergrund geladen werden, regelmäßig nachsehen:
  // anfangs schnell, dann seltener (große Importe dauern), nicht im Hintergrund-Tab.
  let polls = 0;
  let visibleTab = $state(document.visibilityState === "visible");
  $effect(() => {
    if (!visibleTab) return;
    if (!bookmarks.some((b) => b.meta_status === "pending")) {
      polls = 0;
      return;
    }
    const delay = Math.min(1500 * 1.2 ** polls, 20_000);
    const timer = setTimeout(() => {
      polls += 1;
      reload(true);
    }, delay);
    return () => clearTimeout(timer);
  });

  // Gelöschter oder unbekannter Ordner in der Adresse: zur Übersicht.
  $effect(() => {
    if (library && route.kind === "folder" && !current) navigate({ kind: "all" }, true);
  });

  function navigate(target: Route, replace = false) {
    menuOpen = false;
    query = "";
    if (sameRoute(target, route)) return;
    route = target;
    const path = routePath(target);
    if (replace) history.replaceState(null, "", path);
    else history.pushState(null, "", path);
    window.scrollTo(0, 0);
  }

  function linkClick(target: Route, event: MouseEvent) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }
    event.preventDefault();
    navigate(target);
  }

  function folderDeleted(folder: Folder) {
    if (route.kind === "folder" && route.id === folder.id) {
      navigate(
        folder.parent_id === null ? { kind: "all" } : { kind: "folder", id: folder.parent_id },
        true,
      );
    }
  }

  function setView(mode: ViewMode) {
    view = mode;
    savePref("view", mode);
  }

  function setSort(mode: SortMode) {
    sort = mode;
    savePref("sort", mode);
  }

  function keydown(event: KeyboardEvent) {
    // Escape beendet zuerst die Auswahl (Menü und Dialoge fangen es selbst ab).
    if (event.key === "Escape" && selection.kind && !dialog && !menu.open) {
      selection.clear();
      return;
    }
    if (event.key === "Escape" && menuOpen) {
      menuOpen = false;
      // Fokus zurück auf den Menü-Knopf statt ins Leere.
      // Erst nach dem Aktualisieren: Vorher ist der Inhalt noch inert.
      tick().then(() => document.querySelector<HTMLElement>("button.menu")?.focus());
    }
    // „/“ springt in die Suche
    const typing = (event.target as Element).closest("input, textarea, select, [contenteditable]");
    if (event.key === "/" && !typing && !dialog && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      searchInput?.focus();
    }
  }

  function searchKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      query = "";
      searchInput?.blur();
    } else if (event.key === "Enter" && visible[0]) {
      // Ersten Treffer öffnen
      window.open(visible[0].url, "_blank", "noopener,noreferrer");
    }
  }

  // ---- Verschieben per Ziehen ----

  let menuOpenedByDrag = $state(false);

  drag.onstart = () => {
    // Am Handy liegen die Ordner im Menü: beim Ziehen aufklappen.
    if (!menuOpen && matchMedia("(max-width: 800px)").matches) {
      menuOpen = true;
      menuOpenedByDrag = true;
    }
  };

  $effect(() => {
    if (!drag.item && menuOpenedByDrag) {
      menuOpenedByDrag = false;
      menuOpen = false;
    }
  });

  drag.ondrop = (item, target) => {
    move(item, target);
  };

  /** Gezogene Bookmarks: alle ausgewählten, wenn eines davon gezogen wird. */
  function draggedIds(item: DragItem): number[] {
    return selection.is("bookmark") && selection.ids.has(item.id) ? [...selection.ids] : [item.id];
  }

  async function move(item: DragItem, target: DropTarget) {
    try {
      if (item.kind === "bookmark") {
        if (target === "root" || typeof target === "object") return;
        const ids = draggedIds(item);
        if (target === "favorites") {
          await setFavorite(ids, true);
          return;
        }
        const label = ids.length === 1 ? `„${item.label}“` : `${ids.length} Bookmarks`;
        const folderId = target === "unsorted" ? null : target;
        if (ids.length === 1) {
          const bookmark = bookmarks.find((b) => b.id === item.id);
          if (!bookmark || bookmark.folder_id === folderId) return;
          await api.updateBookmark(item.id, { folder_id: folderId });
        } else {
          await api.bulkBookmarks(ids, { action: "move", folder_id: folderId });
          selection.clear();
        }
        toasts.show(`${label} → ${targetName(target)}`);
      } else {
        if (target === "favorites") return;
        if (typeof target === "object") {
          await placeFolder(item, target);
          return;
        }
        // Unsortiert und „oberste Ebene“ bedeuten für Ordner dasselbe.
        const parent = typeof target === "number" ? target : null;
        const folder = tree.byId.get(item.id);
        // Auf sich selbst oder an die alte Stelle gezogen: nichts zu tun.
        if (!folder || folder.parent_id === parent || parent === item.id) return;
        if (parent !== null && subtree(tree, item.id).has(parent)) {
          toasts.show("Ein Ordner kann nicht in sich selbst liegen.", true);
          return;
        }
        await api.updateFolder(item.id, { parent_id: parent });
        toasts.show(
          `Ordner „${item.label}“ → ${parent === null ? "oberste Ebene" : folderName(parent)}`,
        );
      }
    } catch (e) {
      toasts.show(errorText(e), true);
    }
    await reload();
  }

  /** Ordner vor oder hinter einen anderen ziehen (auch in eine andere Ebene). */
  async function placeFolder(
    item: DragItem & { kind: "folder" },
    target: { place: "before" | "after"; id: number },
  ) {
    const folder = tree.byId.get(item.id);
    const ref = tree.byId.get(target.id);
    if (!folder || !ref || ref.id === folder.id) return;
    // Wie in buildTree: unbekannter Elternordner = oberste Ebene.
    const parent = ref.parent_id !== null && tree.byId.has(ref.parent_id) ? ref.parent_id : null;
    if (parent !== null && subtree(tree, item.id).has(parent)) {
      toasts.show("Ein Ordner kann nicht in sich selbst liegen.", true);
      return;
    }
    const current = (tree.children.get(parent) ?? []).map((f) => f.id);
    const ids = current.filter((id) => id !== item.id);
    ids.splice(ids.indexOf(ref.id) + (target.place === "after" ? 1 : 0), 0, item.id);
    if (ids.length === current.length && ids.every((id, index) => id === current[index])) return;
    try {
      await api.orderFolders(parent, ids);
      if (folder.parent_id !== parent)
        toasts.show(
          `Ordner „${item.label}“ → ${parent === null ? "oberste Ebene" : folderName(parent)}`,
        );
    } catch (e) {
      toasts.show(errorText(e), true);
    }
    await reload();
  }

  // ---- Favoriten, Kontextmenüs, Mehrfachauswahl ----

  async function setFavorite(ids: number[], favorite: boolean) {
    try {
      const [only] = ids;
      if (ids.length === 1 && only !== undefined) await api.updateBookmark(only, { favorite });
      else await api.bulkBookmarks(ids, { action: favorite ? "favorite" : "unfavorite" });
      const what = ids.length === 1 ? "Bookmark" : `${ids.length} Bookmarks`;
      toasts.show(favorite ? `${what} als Favorit markiert` : `${what} aus Favoriten entfernt`);
    } catch (e) {
      toasts.show(errorText(e), true);
    }
    await reload();
  }

  // Zwischenablage gibt es nur über HTTPS oder localhost.
  const canCopy = typeof navigator.clipboard?.writeText === "function";

  async function copyLink(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      toasts.show("Link kopiert");
    } catch {
      toasts.show("Kein Zugriff auf die Zwischenablage.", true);
    }
  }

  function bookmarkMenu(bookmark: Bookmark, event: MouseEvent, trigger?: HTMLElement) {
    const items: MenuItem[] = [
      {
        label: "In neuem Tab öffnen",
        icon: "external",
        // URL ist vom Server geprüft (nur http/https).
        action: () => window.open(bookmark.url, "_blank", "noopener,noreferrer"),
      },
      { label: "Bearbeiten", icon: "edit", action: () => (dialog = { kind: "edit", bookmark }) },
      {
        label: bookmark.favorite ? "Favorit entfernen" : "Als Favorit",
        icon: "star",
        filled: bookmark.favorite,
        action: () => setFavorite([bookmark.id], !bookmark.favorite),
      },
      {
        label: "Verschieben nach …",
        icon: "move",
        action: () =>
          (dialog = { kind: "move", target: "bookmark", ids: [bookmark.id], bulk: false }),
      },
    ];
    if (canCopy) {
      items.push({ label: "Link kopieren", icon: "copy", action: () => copyLink(bookmark.url) });
    }
    if (!selection.is("bookmark")) {
      items.push({
        label: "Auswählen",
        icon: "select",
        action: () => selection.start("bookmark", bookmark.id),
      });
    }
    items.push({
      label: "Löschen",
      icon: "trash",
      danger: true,
      action: () => (dialog = { kind: "deleteBookmarks", ids: [bookmark.id], bulk: false }),
    });
    menu.show(event, items, trigger);
  }

  function folderMenu(folder: Folder, event: MouseEvent, trigger?: HTMLElement) {
    const items: MenuItem[] = [];
    if (!(route.kind === "folder" && route.id === folder.id)) {
      items.push({
        label: "Öffnen",
        icon: "folder",
        action: () => navigate({ kind: "folder", id: folder.id }),
      });
    }
    items.push(
      {
        label: "Neuer Unterordner",
        icon: "folderPlus",
        action: () => (dialog = { kind: "newFolder", parentId: folder.id }),
      },
      { label: "Bearbeiten", icon: "edit", action: () => (dialog = { kind: "editFolder", folder }) },
      {
        label: "Verschieben nach …",
        icon: "move",
        action: () => (dialog = { kind: "move", target: "folder", ids: [folder.id], bulk: false }),
      },
    );
    if (!selection.is("folder")) {
      items.push({
        label: "Auswählen",
        icon: "select",
        action: () => {
          selection.start("folder", folder.id);
          // Am Handy liegen die Ordner im Menü: offen lassen bzw. öffnen.
          if (matchMedia("(max-width: 800px)").matches) menuOpen = true;
        },
      });
    }
    items.push({
      label: "Löschen",
      icon: "trash",
      danger: true,
      action: () => (dialog = { kind: "deleteFolders", ids: [folder.id], bulk: false }),
    });
    menu.show(event, items, trigger);
  }

  function folderBulk(action: "merge" | "move" | "delete") {
    const ids = [...selection.ids];
    if (ids.length === 0) return;
    if (action === "merge") dialog = { kind: "merge", ids };
    else if (action === "move") dialog = { kind: "move", target: "folder", ids, bulk: true };
    else dialog = { kind: "deleteFolders", ids, bulk: true };
  }

  /** Klick im Auswahlmodus; mit Umschalt der ganze Bereich seit dem letzten Klick. */
  function selectBookmark(bookmark: Bookmark, event: MouseEvent) {
    const anchor = selection.anchor;
    if (event.shiftKey && anchor !== null) {
      const from = visible.findIndex((b) => b.id === anchor);
      const to = visible.findIndex((b) => b.id === bookmark.id);
      if (from >= 0 && to >= 0) {
        const [first, last] = from < to ? [from, to] : [to, from];
        selection.add(visible.slice(first, last + 1).map((b) => b.id));
        selection.anchor = bookmark.id;
        return;
      }
    }
    selection.toggle(bookmark.id);
  }

  const selectingBookmarks = $derived(selection.is("bookmark"));
  const selectedBookmarks = $derived(
    selectingBookmarks ? bookmarks.filter((b) => selection.ids.has(b.id)) : [],
  );
  const allSelected = $derived(
    visible.length > 0 && visible.every((b) => selection.ids.has(b.id)),
  );
  const allFavorite = $derived(
    selectedBookmarks.length > 0 && selectedBookmarks.every((b) => b.favorite),
  );

  // Ordner, Suche oder Sortierung gewechselt: Auswahl der Bookmarks beenden.
  $effect(() => {
    void routePath(route);
    void query;
    untrack(() => {
      if (selection.is("bookmark")) selection.clear();
    });
  });

  // Nach dem Neuladen: nicht mehr vorhandene Einträge aus der Auswahl nehmen.
  $effect(() => {
    if (!library) return;
    const ids = new Set(
      selection.kind === "folder"
        ? library.folders.map((f) => f.id)
        : library.bookmarks.map((b) => b.id),
    );
    untrack(() => selection.keep((id) => ids.has(id)));
  });

  // Unten liegt die Aktionsleiste: Hinweise darüber anzeigen.
  $effect(() => {
    document.documentElement.classList.toggle("has-selectbar", selectingBookmarks);
  });

  async function deleteBookmarks(ids: number[], bulk: boolean): Promise<boolean> {
    try {
      const [only] = ids;
      if (ids.length === 1 && only !== undefined) await api.deleteBookmark(only);
      else await api.bulkBookmarks(ids, { action: "delete" });
      toasts.show(ids.length === 1 ? "Bookmark gelöscht" : `${ids.length} Bookmarks gelöscht`);
      if (bulk) selection.clear();
      await reload();
      return true;
    } catch (e) {
      toasts.show(errorText(e), true);
      await reload();
      return false;
    }
  }

  function afterDialog(bulk: boolean) {
    if (bulk) selection.clear();
    reload();
  }

  function deleteMessage(ids: number[]): string {
    if (ids.length > 1) return `${ids.length} Bookmarks werden gelöscht.`;
    const bookmark = bookmarks.find((b) => b.id === ids[0]);
    return `„${bookmark?.title ?? "Bookmark"}“ wird gelöscht.`;
  }

  // ---- Links von außen hineinziehen ----

  let externalTimer: ReturnType<typeof setTimeout> | undefined;

  function endExternal() {
    external = false;
    drag.target = null;
  }

  function dragover(event: DragEvent) {
    if (drag.item || !hasLink(event.dataTransfer)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    external = true;
    drag.target = targetAt(event.clientX, event.clientY);
    // Kein zuverlässiges „dragleave“ für das Fenster: Zeitgeber statt dessen.
    clearTimeout(externalTimer);
    externalTimer = setTimeout(endExternal, 250);
  }

  function drop(event: DragEvent) {
    if (!external || !event.dataTransfer) return;
    event.preventDefault();
    clearTimeout(externalTimer);
    const target = externalTarget(drag.target);
    endExternal();
    const links = droppedLinks(event.dataTransfer);
    if (links.length === 0) {
      toasts.show("Kein Link gefunden. Nur http- und https-Adressen.", true);
      return;
    }
    saveLinks(links, target);
  }

  /** Ordner für Links von außen; ohne passendes Ziel der aktuelle Ordner. */
  function externalTarget(value: string | null): number | null {
    const target = parseTarget(value);
    if (typeof target === "number") return target;
    return target === "unsorted" ? null : currentFolderId;
  }

  async function saveLinks(links: DroppedLink[], folder: number | null) {
    const existing = new Set(bookmarks.map((b) => b.url));
    let saved = 0;
    let duplicates = 0;
    let failed = 0;
    for (const link of links) {
      if (existing.has(link.url)) {
        duplicates += 1;
        continue;
      }
      try {
        await api.createBookmark({ url: link.url, title: link.title, folder_id: folder });
        saved += 1;
      } catch {
        failed += 1;
      }
    }
    await reload();
    const parts = [];
    if (saved > 0) {
      parts.push(
        `${saved === 1 ? "Bookmark" : `${saved} Bookmarks`} gespeichert in ${folderName(folder)}`,
      );
    }
    if (duplicates > 0) parts.push(`${duplicates} schon vorhanden`);
    if (failed > 0) parts.push(`${failed} fehlgeschlagen`);
    toasts.show(parts.join(", "), saved === 0);
  }

  // Natives Ziehen von Elementen der eigenen Seite (z. B. Ordner-Links) unterbinden,
  // sonst würde SEBO seine eigenen Adressen als Bookmark speichern.
  function dragstart(event: DragEvent) {
    event.preventDefault();
  }

  const VIEW_LABELS: Record<ViewMode, [string, IconName]> = {
    cards: ["Karten", "cards"],
    list: ["Liste", "list"],
    compact: ["Kompakt", "compact"],
  };

  const SORT_LABELS: Record<SortMode, string> = {
    new: "Neueste zuerst",
    old: "Älteste zuerst",
    name: "Nach Name",
  };

  reload();
</script>

<svelte:document
  onvisibilitychange={() => {
    visibleTab = document.visibilityState === "visible";
    if (visibleTab) reload(true);
  }}
/>

<svelte:window
  onpopstate={() => {
    route = parseRoute(location.pathname);
    query = "";
  }}
  onkeydown={keydown}
  ondragover={dragover}
  ondrop={drop}
  ondragstart={dragstart}
/>

<a class="skip" href="#inhalt" inert={menuOpen}>Zum Inhalt springen</a>

<div class="layout" class:menu-open={menuOpen}>
  <aside id="sidebar">
    <Sidebar
      {tree}
      {route}
      {counts}
      total={bookmarks.length}
      unsorted={unsortedCount}
      favorites={favoritesCount}
      onnavigate={(target) => navigate(target)}
      onnewfolder={() => (dialog = { kind: "newFolder", parentId: currentFolderId })}
      onfoldermenu={folderMenu}
      onfolderbulk={folderBulk}
      {onlogout}
      onimported={reload}
    />
  </aside>
  <button
    type="button"
    class="backdrop"
    tabindex="-1"
    aria-label="Menü schließen"
    onclick={() => (menuOpen = false)}
  ></button>

  <div class="content" inert={menuOpen && !menuOpenedByDrag}>
    <header class="toolbar">
      <button
        type="button"
        class="icon menu"
        aria-label="Menü"
        aria-controls="sidebar"
        aria-expanded={menuOpen}
        onclick={() => {
          menuOpen = !menuOpen;
          // Tastatur: Fokus ins Menü, damit Tab nicht im verdeckten Inhalt landet.
          if (menuOpen) tick().then(() => document.querySelector<HTMLElement>("#sidebar a")?.focus());
        }}
      >
        <Icon name="menu" />
      </button>

      <div class="heading">
        {#if breadcrumbs.length > 0}
          <nav class="crumbs" aria-label="Pfad">
            {#each breadcrumbs as crumb (crumb.id)}
              {@const target = { kind: "folder", id: crumb.id } as const}
              <a href={routePath(target)} onclick={(event) => linkClick(target, event)}>{crumb.name}</a>
              <span aria-hidden="true">/</span>
            {/each}
          </nav>
        {/if}
        <h1>{heading}</h1>
      </div>

      {#if current && !searching}
        <button
          type="button"
          class="icon"
          aria-label="Aktionen für diesen Ordner"
          aria-haspopup="menu"
          onclick={(event) => current && folderMenu(current, event, event.currentTarget)}
        >
          <Icon name="more" />
        </button>
      {/if}

      <button
        type="button"
        class="add"
        aria-label="Hinzufügen"
        onclick={() => (dialog = { kind: "add" })}
      >
        <Icon name="plus" />
        <span>Hinzufügen</span>
      </button>

      <div class="search">
        <Icon name="search" />
        <input
          type="search"
          placeholder="Suchen …"
          aria-label="Bookmarks durchsuchen"
          autocomplete="off"
          spellcheck="false"
          enterkeyhint="go"
          bind:value={query}
          bind:this={searchInput}
          onkeydown={searchKeydown}
        />
      </div>

      <div class="options">
        <div class="views" role="group" aria-label="Ansicht">
          {#each VIEW_MODES as mode (mode)}
            <button
              type="button"
              class="icon"
              class:active={view === mode}
              aria-pressed={view === mode}
              aria-label={VIEW_LABELS[mode][0]}
              title={VIEW_LABELS[mode][0]}
              onclick={() => setView(mode)}
            >
              <Icon name={VIEW_LABELS[mode][1]} />
            </button>
          {/each}
        </div>
        <select
          aria-label="Sortierung"
          value={sort}
          disabled={searching}
          onchange={(event) => setSort(event.currentTarget.value as SortMode)}
        >
          {#each SORT_MODES as mode (mode)}
            <option value={mode}>{SORT_LABELS[mode]}</option>
          {/each}
        </select>
      </div>
    </header>

    <main id="inhalt" tabindex="-1" class:selecting={selectingBookmarks}>
      {#if loadError}
        <div class="notice" role="alert">
          <span>{loadError}</span>
          <button type="button" class="secondary" onclick={() => reload()}>Erneut versuchen</button>
        </div>
      {/if}

      {#if searching && result.total > hits.length}
        <p class="limit">
          Die besten {hits.length} von {result.total} Treffern. Für weitere die Suche verfeinern.
        </p>
      {/if}

      {#if subfolders.length > 0}
        <ul class="subfolders">
          {#each subfolders as folder (folder.id)}
            {@const target = { kind: "folder", id: folder.id } as const}
            <li>
              <a
                href={routePath(target)}
                data-drop={String(folder.id)}
                class:drop-over={drag.target === String(folder.id)}
                onclick={(event) => linkClick(target, event)}
                oncontextmenu={(event) => folderMenu(folder, event)}
              >
                <FolderIcon name={folder.icon} color={folder.color} />
                <span>{folder.name}</span>
              </a>
            </li>
          {/each}
        </ul>
      {/if}

      {#if library === null && !loadError}
        <p class="empty">Lade …</p>
      {:else if library !== null && visible.length === 0}
        <div class="empty">
          {#if searching}
            <p>Keine Treffer für „{query.trim()}“.</p>
          {:else if route.kind === "favorites"}
            <p>Noch keine Favoriten.</p>
            <p class="hint">
              Über ⋯ oder Rechtsklick „Als Favorit“ wählen oder ein Bookmark auf „Favoriten“ ziehen.
            </p>
            <a
              class="button"
              href={routePath({ kind: "all" })}
              onclick={(event) => linkClick({ kind: "all" }, event)}
            >
              Alle Bookmarks
            </a>
          {:else}
            <p>{route.kind === "all" ? "Noch keine Bookmarks." : "Hier sind keine Bookmarks."}</p>
            <p class="hint">Ziehe einen Link hierher oder füge ihn hinzu.</p>
            <button type="button" onclick={() => (dialog = { kind: "add" })}>
              Bookmark hinzufügen
            </button>
          {/if}
        </div>
      {:else}
        <BookmarkView
          bookmarks={visible}
          {view}
          {highlights}
          folderName={showFolders ? folderName : undefined}
          resetKey={`${routePath(route)}|${query}|${sort}`}
          selecting={selectingBookmarks}
          selected={selection.ids}
          onselect={selectBookmark}
          onstart={(bookmark) => selection.start("bookmark", bookmark.id)}
          onmenu={bookmarkMenu}
        />
      {/if}
    </main>
  </div>
</div>

{#if selectingBookmarks}
  {@const count = selection.count}
  <div class="selectbar" role="toolbar" aria-label="Ausgewählte Bookmarks">
    <span class="selected-count" aria-live="polite">{count} ausgewählt</span>
    <button
      type="button"
      class="secondary"
      onclick={() => selection.set(allSelected ? [] : visible.map((b) => b.id))}
    >
      {allSelected ? "Keine" : "Alle"}
    </button>
    <button
      type="button"
      class="icon"
      aria-label={allFavorite ? "Aus Favoriten entfernen" : "Als Favorit"}
      title={allFavorite ? "Aus Favoriten entfernen" : "Als Favorit"}
      disabled={count === 0}
      onclick={() => setFavorite([...selection.ids], !allFavorite)}
    >
      <Icon name="star" filled={allFavorite} />
    </button>
    <button
      type="button"
      class="icon"
      aria-label="Verschieben"
      title="Verschieben"
      disabled={count === 0}
      onclick={() =>
        (dialog = { kind: "move", target: "bookmark", ids: [...selection.ids], bulk: true })}
    >
      <Icon name="move" />
    </button>
    <button
      type="button"
      class="icon danger-text"
      aria-label="Löschen"
      title="Löschen"
      disabled={count === 0}
      onclick={() => (dialog = { kind: "deleteBookmarks", ids: [...selection.ids], bulk: true })}
    >
      <Icon name="trash" />
    </button>
    <button
      type="button"
      class="icon"
      aria-label="Auswahl beenden"
      title="Auswahl beenden (Esc)"
      onclick={() => selection.clear()}
    >
      <Icon name="close" />
    </button>
  </div>
{/if}

{#if drag.item}
  {@const ids = drag.item.kind === "bookmark" ? draggedIds(drag.item) : []}
  <div class="ghost" style:transform="translate({drag.x + 14}px, {drag.y + 14}px)">
    {#if drag.item.kind === "folder"}
      <FolderIcon name={drag.item.icon} color={drag.item.color} />
    {:else}
      <Icon name="all" />
    {/if}
    <span>{ids.length > 1 ? `${ids.length} Bookmarks` : drag.item.label}</span>
  </div>
{/if}

{#if external}
  <div class="drop-hint" aria-hidden="true">
    Loslassen zum Speichern in
    <strong>{folderName(externalTarget(drag.target))}</strong>
  </div>
{/if}

<Toasts />
<ContextMenu />

{#if dialog?.kind === "add"}
  <BookmarkDialog
    {tree}
    {bookmarks}
    folderId={currentFolderId}
    initialUrl={dialog.url}
    initialTitle={dialog.title}
    favorite={route.kind === "favorites"}
    onclose={() => (dialog = null)}
    onchanged={reload}
  />
{:else if dialog?.kind === "edit"}
  <BookmarkDialog
    {tree}
    {bookmarks}
    bookmark={dialog.bookmark}
    onclose={() => (dialog = null)}
    onchanged={reload}
  />
{:else if dialog?.kind === "newFolder"}
  <FolderDialog
    {tree}
    {bookmarks}
    parentId={dialog.parentId}
    onclose={() => (dialog = null)}
    onchanged={reload}
    ondeleted={folderDeleted}
  />
{:else if dialog?.kind === "editFolder"}
  <FolderDialog
    {tree}
    {bookmarks}
    folder={dialog.folder}
    onclose={() => (dialog = null)}
    onchanged={reload}
    ondeleted={folderDeleted}
  />
{:else if dialog?.kind === "move"}
  {@const bulk = dialog.bulk}
  <MoveDialog
    {tree}
    {bookmarks}
    kind={dialog.target}
    ids={dialog.ids}
    onclose={() => (dialog = null)}
    onchanged={() => afterDialog(bulk)}
  />
{:else if dialog?.kind === "deleteBookmarks"}
  {@const { ids, bulk } = dialog}
  <ConfirmDialog
    title={ids.length === 1 ? "Bookmark löschen" : `${ids.length} Bookmarks löschen`}
    message={deleteMessage(ids)}
    confirmLabel="Löschen"
    onconfirm={() => deleteBookmarks(ids, bulk)}
    onclose={() => (dialog = null)}
  />
{:else if dialog?.kind === "deleteFolders"}
  {@const bulk = dialog.bulk}
  <DeleteFoldersDialog
    {tree}
    {bookmarks}
    ids={dialog.ids}
    onclose={() => (dialog = null)}
    onchanged={() => afterDialog(bulk)}
    ondeleted={(ids) => {
      const folder = ids.length === 1 ? tree.byId.get(ids[0] ?? -1) : undefined;
      if (folder) folderDeleted(folder);
    }}
  />
{:else if dialog?.kind === "merge"}
  <MergeDialog
    {tree}
    ids={dialog.ids}
    onclose={() => (dialog = null)}
    onchanged={() => afterDialog(true)}
    onmerged={(target, sources) => {
      if (route.kind === "folder" && sources.includes(route.id)) {
        navigate({ kind: "folder", id: target }, true);
      }
    }}
  />
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: 16.5rem minmax(0, 1fr);
    min-height: 100dvh;
  }

  aside {
    position: sticky;
    top: 0;
    height: 100dvh;
    overflow-y: auto;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }

  .backdrop {
    display: none;
  }

  .content {
    min-width: 0;
  }

  .toolbar {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    padding: 0.75rem 1.5rem;
    background: color-mix(in srgb, var(--bg) 92%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--border);
  }

  .menu {
    display: none;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  .crumbs {
    display: flex;
    gap: 0.35rem;
    font-size: 0.8rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
  }

  .crumbs a {
    color: inherit;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  h1 {
    margin: 0;
    font-size: 1.35rem;
    line-height: 1.25;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .add {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
    width: min(18rem, 30vw);
    color: var(--muted);
  }

  .search :global(svg) {
    position: absolute;
    left: 0.65rem;
    pointer-events: none;
  }

  .search input {
    padding-left: 2.25rem;
    background: var(--surface);
  }

  .hint {
    font-size: 0.9rem;
  }

  .limit {
    margin: 0 0 1rem;
    color: var(--muted);
    font-size: 0.9rem;
  }

  main:focus {
    outline: none;
  }

  .skip {
    position: absolute;
    top: 0.5rem;
    left: 0.5rem;
    z-index: 100;
    padding: 0.5rem 1rem;
    color: var(--accent-text);
    background: var(--accent);
    border-radius: calc(var(--radius) - 4px);
    transform: translateY(-200%);
  }

  .skip:focus {
    transform: none;
  }

  .ghost {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    max-width: 16rem;
    padding: 0.4rem 0.7rem;
    font-size: 0.9rem;
    font-weight: 600;
    background: var(--surface);
    border: 1px solid var(--accent);
    border-radius: calc(var(--radius) - 4px);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.25);
    pointer-events: none;
  }

  .ghost span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .drop-hint {
    position: fixed;
    inset: auto 0 0;
    z-index: 40;
    padding: 1rem calc(1rem + env(safe-area-inset-right)) calc(1rem + env(safe-area-inset-bottom));
    text-align: center;
    color: var(--accent-text);
    background: var(--accent);
    pointer-events: none;
  }

  .options {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .views {
    display: flex;
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) - 4px);
    overflow: hidden;
  }

  .views button {
    border-radius: 0;
    color: var(--muted);
  }

  .views button.active {
    color: var(--text);
    background: var(--selected);
  }

  main {
    padding: 1.25rem 1.5rem 3rem;
  }

  .notice {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 1rem;
    padding: 0.6rem 0.75rem;
    color: var(--danger);
    border: 1px solid currentColor;
    border-radius: var(--radius);
  }

  .subfolders {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin: 0 0 1.25rem;
    padding: 0;
    list-style: none;
  }

  .subfolders a {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.75rem;
    color: inherit;
    text-decoration: none;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 999px;
  }

  .subfolders a:hover {
    background: var(--hover);
  }

  .empty {
    margin-top: 15vh;
    text-align: center;
    color: var(--muted);
  }

  /* Platz für die Aktionsleiste, damit die letzten Einträge erreichbar bleiben */
  main.selecting {
    padding-bottom: 7rem;
  }

  .selectbar {
    position: fixed;
    left: 50%;
    bottom: calc(1rem + env(safe-area-inset-bottom));
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    width: max-content;
    max-width: calc(100% - 2rem);
    padding: 0.4rem 0.4rem 0.4rem 1rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 999px;
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.22);
    transform: translateX(-50%);
  }

  .selected-count {
    margin-right: 0.5rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .selectbar .secondary {
    padding: 0.35rem 0.8rem;
    border-radius: 999px;
  }

  .selectbar .danger-text {
    color: var(--danger);
  }

  /* Breite Seitenleiste: Leiste über dem Inhalt mittig */
  @media (min-width: 801px) {
    .selectbar {
      left: calc(50% + 8.25rem);
    }
  }

  /* Handy und schmale Fenster: Seitenleiste als ausklappbares Menü. */
  @media (max-width: 800px) {
    .layout {
      display: block;
    }

    aside {
      position: fixed;
      inset: 0 auto 0 0;
      z-index: 20;
      width: min(85vw, 18rem);
      transform: translateX(-100%);
      transition: transform 0.2s ease;
      visibility: hidden;
    }

    .menu-open aside {
      transform: none;
      visibility: visible;
      box-shadow: 0 0 40px rgb(0 0 0 / 0.3);
    }

    .menu-open .backdrop {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 15;
      width: 100%;
      padding: 0;
      background: rgb(0 0 0 / 0.4);
      border: none;
      border-radius: 0;
    }

    .menu {
      display: grid;
    }

    .toolbar {
      position: static;
      padding: 0.5rem 1rem;
    }

    .search {
      order: 10;
      width: 100%;
    }

    .options {
      order: 11;
    }

    .add span {
      display: none;
    }

    .add {
      padding: 0.5rem;
    }

    .options {
      width: 100%;
      justify-content: space-between;
    }

    main {
      padding: 1rem 1rem 3rem;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    aside {
      transition: none;
    }
  }
</style>
