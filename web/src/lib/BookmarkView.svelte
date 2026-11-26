<script lang="ts">
  import type { Bookmark } from "./bindings/Bookmark";
  import { draggable, drag } from "./drag.svelte";
  import Icon from "./Icon.svelte";
  import { host, type ViewMode } from "./library";
  import type { Part } from "./search";
  import Thumb from "./Thumb.svelte";

  interface Props {
    bookmarks: Bookmark[];
    view: ViewMode;
    /** Hervorgehobene Titelteile (Suchtreffer). */
    highlights?: Map<number, Part[]>;
    /** Ordnername anzeigen (Übersicht und Suche). */
    folderName?: (id: number | null) => string;
    /** Ändert sich beim Wechsel von Ordner, Suche oder Sortierung: wieder oben anfangen. */
    resetKey: string;
    /** Auswahlmodus: Klick wählt aus, statt den Link zu öffnen. */
    selecting: boolean;
    selected: ReadonlySet<number>;
    onselect: (bookmark: Bookmark, event: MouseEvent) => void;
    /** Kontextmenü: Rechtsklick oder ⋯-Knopf (dann mit `trigger`). */
    onmenu: (bookmark: Bookmark, event: MouseEvent, trigger?: HTMLElement) => void;
  }

  let {
    bookmarks,
    view,
    highlights,
    folderName,
    resetKey,
    selecting,
    selected,
    onselect,
    onmenu,
  }: Props = $props();

  function contextmenu(bookmark: Bookmark, event: MouseEvent) {
    // Langes Drücken am Handy ist Ziehen (dort verhindert), nicht Menü.
    if (event.defaultPrevented || drag.item) return;
    onmenu(bookmark, event);
  }

  function click(bookmark: Bookmark, event: MouseEvent) {
    if (!selecting) return;
    event.preventDefault();
    onselect(bookmark, event);
  }

  // Nur so viele Einträge ins DOM, wie gebraucht werden; beim Scrollen kommen weitere dazu.
  const PAGE = 150;
  let limit = $state(PAGE);
  let sentinel = $state<HTMLElement>();
  const shown = $derived(bookmarks.length > limit ? bookmarks.slice(0, limit) : bookmarks);

  $effect(() => {
    void resetKey;
    limit = PAGE;
  });

  // Bei jeder Erhöhung neu beobachten: Ist die Markierung dann noch sichtbar,
  // meldet der neue Observer das sofort und es wird weiter nachgeladen.
  $effect(() => {
    void limit;
    if (!sentinel) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) limit += PAGE;
      },
      { rootMargin: "1200px 0px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  });
</script>

<ul class={view}>
  {#each shown as bookmark (bookmark.id)}
    {@const parts = highlights?.get(bookmark.id)}
    {@const isSelected = selecting && selected.has(bookmark.id)}
    <li
      use:draggable={{ kind: "bookmark", id: bookmark.id, label: bookmark.title }}
      class:dragged={drag.item?.kind === "bookmark" &&
        (drag.item.id === bookmark.id || (isSelected && selected.has(drag.item.id)))}
      class:selected={isSelected}
      oncontextmenu={(event) => contextmenu(bookmark, event)}
    >
      <!-- URL ist vom Server geprüft (nur http/https). -->
      <a
        href={bookmark.url}
        target="_blank"
        rel="noopener noreferrer"
        title={bookmark.url}
        role={selecting ? "checkbox" : undefined}
        aria-checked={selecting ? isSelected : undefined}
        onclick={(event) => click(bookmark, event)}
      >
        {#if selecting}
          <span class="check" class:on={isSelected} aria-hidden="true">
            <Icon name="check" />
          </span>
        {/if}
        <span class="visual">
          <Thumb {bookmark} kind={view === "cards" ? "image" : "icon"} />
        </span>
        <span class="text">
          <span class="title">
            {#if parts}
              {#each parts as part, index (index)}
                {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
              {/each}
            {:else}
              {bookmark.title}
            {/if}
          </span>
          <span class="host">
            {#if bookmark.favorite}
              <span class="fav" title="Favorit"><Icon name="star" filled /></span>
            {/if}
            {#if view === "cards" && bookmark.image && bookmark.icon}
              <img class="favicon" src={bookmark.icon} alt="" loading="lazy" decoding="async" />
            {/if}
            <span class="domain">{host(bookmark.url)}</span>
            {#if folderName && view === "list"}
              <span class="folder">· {folderName(bookmark.folder_id)}</span>
            {/if}
            {#if bookmark.meta_status === "pending"}
              <span class="pending">· lädt …</span>
            {/if}
          </span>
        </span>
      </a>
      {#if !selecting}
        <button
          type="button"
          class="edit icon"
          aria-label="Aktionen für „{bookmark.title}“"
          aria-haspopup="menu"
          onclick={(event) => onmenu(bookmark, event, event.currentTarget)}
        >
          <Icon name="more" />
        </button>
      {/if}
    </li>
  {/each}
</ul>
{#if shown.length < bookmarks.length}
  <div class="more" bind:this={sentinel}>{bookmarks.length - shown.length} weitere …</div>
{/if}

<style>
  .more {
    padding: 1.5rem;
    text-align: center;
    color: var(--muted);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    position: relative;
    min-width: 0;
    /* Lange Druck-Geste fürs Ziehen statt Textauswahl und Link-Menü */
    -webkit-touch-callout: none;
    user-select: none;
  }

  li.dragged {
    opacity: 0.4;
  }

  li.selected {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .cards li.selected,
  .list li.selected,
  .compact li.selected {
    background: var(--selected);
  }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 1.5rem;
    height: 1.5rem;
    color: transparent;
    background: var(--surface);
    border: 2px solid var(--muted);
    border-radius: 50%;
  }

  .check :global(svg) {
    width: 16px;
    height: 16px;
    stroke-width: 3;
  }

  .check.on {
    color: var(--accent-text);
    background: var(--accent);
    border-color: var(--accent);
  }

  .cards .check {
    position: absolute;
    top: 0.5rem;
    left: 0.5rem;
    z-index: 1;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.25);
  }

  .fav {
    flex: none;
    display: grid;
    color: #e0a100;
  }

  .fav :global(svg) {
    width: 14px;
    height: 14px;
  }

  mark {
    color: inherit;
    background: var(--mark);
    border-radius: 2px;
  }

  .domain,
  .folder {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  a {
    display: flex;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .host {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .favicon {
    width: 16px;
    height: 16px;
    border-radius: 3px;
  }

  .edit {
    position: absolute;
    color: var(--text);
    background: var(--surface);
  }

  /* Mit Maus nur beim Überfahren zeigen, bei Touch immer. */
  @media (hover: hover) {
    li .edit {
      opacity: 0;
    }

    li:hover .edit,
    li:focus-within .edit {
      opacity: 1;
    }
  }

  /* Karten */
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr));
    gap: 1rem;
  }

  .cards li {
    content-visibility: auto;
    contain-intrinsic-size: auto 15rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .cards a {
    flex-direction: column;
    height: 100%;
  }

  .cards .text {
    padding: 0.6rem 0.75rem 0.75rem;
    gap: 0.2rem;
  }

  .cards .title {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.3;
  }

  .cards .edit {
    top: 0.4rem;
    right: 0.4rem;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.2);
  }

  /* Liste */
  .list,
  .compact {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  /* Lange, weit gescrollte Listen: Layout und Zeichnen nur für Sichtbares */
  .list li,
  .compact li {
    content-visibility: auto;
    contain-intrinsic-size: auto 3.6rem;
  }

  .compact li {
    contain-intrinsic-size: auto 2.2rem;
  }

  .list li + li,
  .compact li + li {
    border-top: 1px solid var(--border);
  }

  .list a {
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 3.25rem 0.6rem 0.75rem;
  }

  .list .visual {
    flex: none;
    width: 2.5rem;
    height: 2.5rem;
    font-size: 2.5rem;
  }

  .list .title {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list .edit,
  .compact .edit {
    top: 50%;
    right: 0.4rem;
    transform: translateY(-50%);
  }

  /* Kompakt */
  .compact a {
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 3rem 0.35rem 0.75rem;
  }

  .compact .visual {
    flex: none;
    width: 1.25rem;
    height: 1.25rem;
    font-size: 1.25rem;
  }

  .compact .text {
    flex-direction: row;
    align-items: baseline;
    gap: 0.75rem;
  }

  .compact .title {
    flex: 0 1 auto;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compact .host {
    flex: 1 10 auto;
    min-width: 4rem;
  }

  .compact .edit {
    padding: 0.3rem;
  }

  @media (max-width: 600px) {
    .compact .host {
      display: none;
    }

    /* Zwei Spalten am Handy */
    .cards {
      grid-template-columns: repeat(auto-fill, minmax(min(100%, 9.5rem), 1fr));
      gap: 0.75rem;
    }

    .cards .text {
      padding: 0.5rem 0.6rem 0.6rem;
    }

    .cards .title {
      font-size: 0.95rem;
    }

    .cards .edit {
      top: 0.3rem;
      right: 0.3rem;
      padding: 0.35rem;
    }
  }
</style>
