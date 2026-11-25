<script lang="ts">
  import { api, errorText } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import type { Folder } from "./bindings/Folder";
  import type { Library } from "./bindings/Library";
  import BookmarkDialog from "./BookmarkDialog.svelte";
  import BookmarkView from "./BookmarkView.svelte";
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
    VIEW_MODES,
    type Route,
    type SortMode,
    type ViewMode,
  } from "./library";
  import Sidebar from "./Sidebar.svelte";

  let { onlogout }: { onlogout: () => void } = $props();

  type Dialog =
    | { kind: "add" }
    | { kind: "edit"; bookmark: Bookmark }
    | { kind: "newFolder" }
    | { kind: "editFolder"; folder: Folder };

  let library = $state<Library | null>(null);
  let loadError = $state("");
  let route = $state<Route>(parseRoute(location.pathname));
  let view = $state<ViewMode>(loadPref("view", VIEW_MODES, "cards"));
  let sort = $state<SortMode>(loadPref("sort", SORT_MODES, "new"));
  let menuOpen = $state(false);
  let dialog = $state<Dialog | null>(null);

  const tree = $derived(buildTree(library?.folders ?? []));
  const bookmarks = $derived(library?.bookmarks ?? []);

  const counts = $derived.by(() => {
    const result = new Map<number, number>();
    for (const b of bookmarks) {
      if (b.folder_id !== null) result.set(b.folder_id, (result.get(b.folder_id) ?? 0) + 1);
    }
    return result;
  });
  const unsortedCount = $derived(bookmarks.filter((b) => b.folder_id === null).length);

  const current = $derived(route.kind === "folder" ? tree.byId.get(route.id) : undefined);
  const breadcrumbs = $derived(current ? folderPath(tree, current.id).slice(0, -1) : []);
  const heading = $derived(
    route.kind === "all" ? "Alle Bookmarks" : route.kind === "unsorted" ? "Unsortiert" : (current?.name ?? ""),
  );
  const subfolders = $derived(current ? (tree.children.get(current.id) ?? []) : []);
  const visible = $derived(
    sortBookmarks(
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
  const currentFolderId = $derived(route.kind === "folder" ? route.id : null);

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

  // Solange Titel und Bilder im Hintergrund geladen werden, regelmäßig nachsehen.
  let polls = 0;
  $effect(() => {
    if (!bookmarks.some((b) => b.meta_status === "pending")) {
      polls = 0;
      return;
    }
    if (polls >= 40) return;
    const timer = setTimeout(() => {
      polls += 1;
      reload();
    }, 1500);
    return () => clearTimeout(timer);
  });

  // Gelöschter oder unbekannter Ordner in der Adresse: zur Übersicht.
  $effect(() => {
    if (library && route.kind === "folder" && !current) navigate({ kind: "all" }, true);
  });

  function navigate(target: Route, replace = false) {
    menuOpen = false;
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
      navigate(folder.parent_id === null ? { kind: "all" } : { kind: "folder", id: folder.parent_id }, true);
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

<svelte:window onpopstate={() => (route = parseRoute(location.pathname))} onkeydown={keydown} />

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
    />
  </aside>
  <button
    type="button"
    class="backdrop"
    tabindex="-1"
    aria-label="Menü schließen"
    onclick={() => (menuOpen = false)}
  ></button>

  <div class="content">
    <header class="toolbar">
      <button
        type="button"
        class="icon menu"
        aria-label="Menü"
        aria-controls="sidebar"
        aria-expanded={menuOpen}
        onclick={() => (menuOpen = !menuOpen)}
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

      {#if current}
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
          onchange={(event) => setSort(event.currentTarget.value as SortMode)}
        >
          {#each SORT_MODES as mode (mode)}
            <option value={mode}>{SORT_LABELS[mode]}</option>
          {/each}
        </select>
      </div>
    </header>

    <main>
      {#if loadError}
        <div class="notice" role="alert">
          <span>{loadError}</span>
          <button type="button" class="secondary" onclick={reload}>Erneut versuchen</button>
        </div>
      {/if}

      {#if subfolders.length > 0}
        <ul class="subfolders">
          {#each subfolders as folder (folder.id)}
            {@const target = { kind: "folder", id: folder.id } as const}
            <li>
              <a href={routePath(target)} onclick={(event) => linkClick(target, event)}>
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
          <p>{route.kind === "all" ? "Noch keine Bookmarks." : "Hier sind keine Bookmarks."}</p>
          <button type="button" onclick={() => (dialog = { kind: "add" })}>Bookmark hinzufügen</button>
        </div>
      {:else}
        <BookmarkView
          bookmarks={visible}
          {view}
          onedit={(bookmark) => (dialog = { kind: "edit", bookmark })}
        />
      {/if}
    </main>
  </div>
</div>

{#if dialog?.kind === "add"}
  <BookmarkDialog {tree} folderId={currentFolderId} onclose={() => (dialog = null)} onchanged={reload} />
{:else if dialog?.kind === "edit"}
  <BookmarkDialog
    {tree}
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
      padding: 0.5rem 1rem;
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
