<script lang="ts">
  import { api, errorText } from "./api";
  import type { Folder } from "./bindings/Folder";
  import { drag } from "./drag.svelte";
  import Footer from "./Footer.svelte";
  import FolderTree from "./FolderTree.svelte";
  import Icon from "./Icon.svelte";
  import { folderPath, routePath, type Route, type Tree } from "./library";
  import { toasts } from "./toast.svelte";

  interface Props {
    tree: Tree;
    route: Route;
    counts: Map<number, number>;
    total: number;
    unsorted: number;
    onnavigate: (route: Route) => void;
    onnewfolder: () => void;
    oneditfolder: (folder: Folder) => void;
    onlogout: () => void;
    /** Nach einem Import, damit die Liste neu geladen wird. */
    onimported: () => void;
  }

  let {
    tree,
    route,
    counts,
    total,
    unsorted,
    onnavigate,
    onnewfolder,
    oneditfolder,
    onlogout,
    onimported,
  }: Props = $props();

  let fileInput = $state<HTMLInputElement>();
  /** Wie im Server (`MAX_IMPORT_BYTES`). */
  const MAX_IMPORT_BYTES = 20 * 1024 * 1024;

  function count(n: number, one: string, many: string) {
    return `${n} ${n === 1 ? one : many}`;
  }

  // Aktueller Ordner liegt in einem zugeklappten Ordner: Eltern aufklappen.
  $effect(() => {
    if (route.kind !== "folder") return;
    const ancestors = folderPath(tree, route.id).slice(0, -1);
    const closed = ancestors.filter((folder) => collapsed.has(folder.id));
    if (closed.length === 0) return;
    const next = new Set(collapsed);
    for (const folder of closed) next.delete(folder.id);
    collapsed = next;
  });
  let importing = $state(false);

  async function importFile(event: Event & { currentTarget: HTMLInputElement }) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file || importing) return;
    if (file.size > MAX_IMPORT_BYTES) {
      toasts.show("Die Datei ist zu groß (höchstens 20 MB).", true);
      return;
    }
    importing = true;
    try {
      const result = await api.importFile(file);
      const parts = [
        `${count(result.bookmarks, "Bookmark", "Bookmarks")} und ${result.folders} Ordner importiert`,
      ];
      if (result.duplicates > 0) parts.push(`${result.duplicates} schon vorhanden`);
      if (result.invalid > 0) parts.push(`${result.invalid} ungültig`);
      if (result.dropped > 0) parts.push(`${result.dropped} wegen Obergrenze weggelassen`);
      toasts.show(parts.join(", "));
      onimported();
    } catch (e) {
      toasts.show(errorText(e), true);
    } finally {
      importing = false;
    }
  }

  const COLLAPSED_KEY = "sebo.collapsed";

  function loadCollapsed(): Set<number> {
    try {
      const stored = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
      return new Set(Array.isArray(stored) ? stored.filter(Number.isInteger) : []);
    } catch {
      return new Set();
    }
  }

  let collapsed = $state(loadCollapsed());

  function toggle(id: number) {
    const next = new Set(collapsed);
    if (!next.delete(id)) next.add(id);
    collapsed = next;
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
    } catch {
      // ohne Speicher eben nicht
    }
  }

  // Normale Klicks innerhalb der App, Strg/Mittelklick öffnet weiterhin einen neuen Tab.
  function navigate(target: Route, event: MouseEvent) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }
    event.preventDefault();
    onnavigate(target);
  }

  const ALL: Route = { kind: "all" };
  const UNSORTED: Route = { kind: "unsorted" };
</script>

<nav aria-label="Ordner">
  <div class="brand">SEBO</div>

  <ul class="fixed">
    <li>
      <a
        href={routePath(ALL)}
        class:active={route.kind === "all"}
        onclick={(event) => navigate(ALL, event)}
      >
        <Icon name="all" />
        <span class="name">Alle Bookmarks</span>
        <span class="count">{total}</span>
      </a>
    </li>
    <li>
      <a
        href={routePath(UNSORTED)}
        class:active={route.kind === "unsorted"}
        class:drop-over={drag.target === "none"}
        data-drop="none"
        onclick={(event) => navigate(UNSORTED, event)}
      >
        <Icon name="inbox" />
        <span class="name">Unsortiert</span>
        <span class="count">{unsorted}</span>
      </a>
    </li>
  </ul>

  <div class="section">
    <span>Ordner</span>
    <button type="button" class="icon" aria-label="Neuer Ordner" onclick={onnewfolder}>
      <Icon name="folderPlus" />
    </button>
  </div>

  {#if tree.children.size === 0}
    <p class="empty">Noch keine Ordner.</p>
  {:else}
    <FolderTree
      {tree}
      parent={null}
      depth={0}
      {route}
      {counts}
      {collapsed}
      ontoggle={toggle}
      onnavigate={navigate}
      onedit={oneditfolder}
    />
  {/if}

  <div class="bottom">
    <div class="transfer">
      <button
        type="button"
        class="secondary"
        disabled={importing}
        onclick={() => fileInput?.click()}
        title="HTML-Exportdatei eines Browsers oder von SEBO"
      >
        {importing ? "Importiere …" : "Importieren"}
      </button>
      <a class="button secondary" href="/api/export" download>Exportieren</a>
      <input
        bind:this={fileInput}
        type="file"
        accept=".html,.htm,text/html"
        hidden
        onchange={importFile}
      />
    </div>
    <button type="button" class="secondary logout" onclick={onlogout}>
      <Icon name="logout" />
      Abmelden
    </button>
    <Footer />
  </div>
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    padding: 0.75rem 0.75rem 0;
  }

  .brand {
    padding: 0.25rem 0.5rem 0.75rem;
    color: var(--accent);
    font-size: 1.2rem;
    font-weight: 700;
  }

  .fixed {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .fixed a {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.5rem;
    color: inherit;
    text-decoration: none;
    border-radius: calc(var(--radius) - 4px);
  }

  .fixed a:hover {
    background: var(--hover);
  }

  .fixed a.active {
    background: var(--selected);
  }

  .name {
    flex: 1;
  }

  .count {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .section {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 1rem 0 0.25rem;
    padding-left: 0.5rem;
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .section button {
    color: var(--muted);
  }

  .empty {
    margin: 0.25rem 0.5rem;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .bottom {
    margin-top: auto;
    padding-top: 1.5rem;
  }

  .transfer {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .transfer > * {
    font-size: 0.9rem;
    font-weight: 500;
    padding-inline: 0.5rem;
  }

  .logout {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    font-weight: 500;
  }

  .bottom :global(footer) {
    padding: 1rem 0 calc(1rem + env(safe-area-inset-bottom));
  }
</style>
