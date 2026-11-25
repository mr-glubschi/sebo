<script lang="ts">
  import { tick } from "svelte";
  import { api, errorText } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import type { Folder } from "./bindings/Folder";
  import type { Library } from "./bindings/Library";
  import BookmarkDialog from "./BookmarkDialog.svelte";
  import BookmarkView from "./BookmarkView.svelte";
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
  import { SearchIndex, type Part } from "./search";
  import { readShare } from "./share";
  import Sidebar from "./Sidebar.svelte";
  import { toasts } from "./toast.svelte";
  import Toasts from "./Toasts.svelte";

  let { onlogout }: { onlogout: () => void } = $props();

  type Dialog =
    | { kind: "add"; url?: string; title?: string }
    | { kind: "edit"; bookmark: Bookmark }
    | { kind: "newFolder" }
    | { kind: "editFolder"; folder: Folder };

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

  const current = $derived(route.kind === "folder" ? tree.byId.get(route.id) : undefined);
  const breadcrumbs = $derived(
    current && !searching ? folderPath(tree, current.id).slice(0, -1) : [],
  );
  const heading = $derived(
    searching
      ? "Suche"
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
  const showFolders = $derived(searching || route.kind === "all");
  const currentFolderId = $derived(route.kind === "folder" ? route.id : null);

  function folderName(id: number | null): string {
    return id === null ? "Unsortiert" : (tree.byId.get(id)?.name ?? "?");
  }

  function targetName(target: DropTarget): string {
    return target === null ? "Unsortiert" : folderName(target);
  }

  $effect(() => {
    document.title = heading ? `${heading} – SEBO` : "SEBO";
  });

  async function reload() {
    try {
      library = await api.library();
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
      reload();
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
    if (event.key === "Escape" && menuOpen) menuOpen = false;
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

  async function move(item: DragItem, target: DropTarget) {
    try {
      if (item.kind === "bookmark") {
        const bookmark = bookmarks.find((b) => b.id === item.id);
        if (!bookmark || bookmark.folder_id === target) return;
        await api.updateBookmark(item.id, { folder_id: target });
        toasts.show(`„${item.label}“ → ${targetName(target)}`);
      } else {
        const folder = tree.byId.get(item.id);
        // Auf sich selbst oder an die alte Stelle gezogen: nichts zu tun.
        if (!folder || folder.parent_id === target || target === item.id) return;
        if (target !== null && subtree(tree, item.id).has(target)) {
          toasts.show("Ein Ordner kann nicht in sich selbst liegen.", true);
          return;
        }
        await api.updateFolder(item.id, { parent_id: target });
        toasts.show(`Ordner „${item.label}“ → ${target === null ? "oberste Ebene" : folderName(target)}`);
      }
    } catch (e) {
      toasts.show(errorText(e), true);
    }
    await reload();
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
    const target = parseTarget(drag.target);
    endExternal();
    const links = droppedLinks(event.dataTransfer);
    if (links.length === 0) {
      toasts.show("Kein Link gefunden. Nur http- und https-Adressen.", true);
      return;
    }
    saveLinks(links, target === undefined ? currentFolderId : target);
  }

  async function saveLinks(links: DroppedLink[], folder: DropTarget) {
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
        `${saved === 1 ? "Bookmark" : `${saved} Bookmarks`} gespeichert in ${targetName(folder)}`,
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
    if (visibleTab) reload();
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

<a class="skip" href="#inhalt">Zum Inhalt springen</a>

<div class="layout" class:menu-open={menuOpen}>
  <aside id="sidebar">
    <Sidebar
      {tree}
      {route}
      {counts}
      total={bookmarks.length}
      unsorted={unsortedCount}
      onnavigate={(target) => navigate(target)}
      onnewfolder={() => (dialog = { kind: "newFolder" })}
      oneditfolder={(folder) => (dialog = { kind: "editFolder", folder })}
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
          aria-label="Ordner bearbeiten"
          onclick={() => current && (dialog = { kind: "editFolder", folder: current })}
        >
          <Icon name="edit" />
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

    <main id="inhalt" tabindex="-1">
      {#if loadError}
        <div class="notice" role="alert">
          <span>{loadError}</span>
          <button type="button" class="secondary" onclick={reload}>Erneut versuchen</button>
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
              >
                <Icon name="folder" />
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
          onedit={(bookmark) => (dialog = { kind: "edit", bookmark })}
          resetKey={`${routePath(route)}|${query}|${sort}`}
        />
      {/if}
    </main>
  </div>
</div>

{#if drag.item}
  <div class="ghost" style:transform="translate({drag.x + 14}px, {drag.y + 14}px)">
    <Icon name={drag.item.kind === "folder" ? "folder" : "all"} />
    <span>{drag.item.label}</span>
  </div>
{/if}

{#if external}
  {@const target = parseTarget(drag.target)}
  <div class="drop-hint" aria-hidden="true">
    Loslassen zum Speichern in
    <strong>{targetName(target === undefined ? currentFolderId : target)}</strong>
  </div>
{/if}

<Toasts />

{#if dialog?.kind === "add"}
  <BookmarkDialog
    {tree}
    {bookmarks}
    folderId={currentFolderId}
    initialUrl={dialog.url}
    initialTitle={dialog.title}
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
    parentId={currentFolderId}
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
