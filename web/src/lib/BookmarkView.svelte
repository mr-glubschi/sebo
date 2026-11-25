<script lang="ts">
  import type { Bookmark } from "./bindings/Bookmark";
  import Icon from "./Icon.svelte";
  import { host, type ViewMode } from "./library";
  import Thumb from "./Thumb.svelte";

  interface Props {
    bookmarks: Bookmark[];
    view: ViewMode;
    onedit: (bookmark: Bookmark) => void;
  }

  let { bookmarks, view, onedit }: Props = $props();
</script>

<ul class={view}>
  {#each bookmarks as bookmark (bookmark.id)}
    <li>
      <!-- URL ist vom Server geprüft (nur http/https). -->
      <a href={bookmark.url} target="_blank" rel="noopener noreferrer" title={bookmark.url}>
        <span class="visual">
          <Thumb {bookmark} kind={view === "cards" ? "image" : "icon"} />
        </span>
        <span class="text">
          <span class="title">{bookmark.title}</span>
          <span class="host">
            {#if view === "cards" && bookmark.image && bookmark.icon}
              <img class="favicon" src={bookmark.icon} alt="" loading="lazy" />
            {/if}
            {host(bookmark.url)}
            {#if bookmark.meta_status === "pending"}
              <span class="pending">· lädt …</span>
            {/if}
          </span>
        </span>
      </a>
      <button
        type="button"
        class="edit icon"
        aria-label="„{bookmark.title}“ bearbeiten"
        onclick={() => onedit(bookmark)}
      >
        <Icon name="edit" />
      </button>
    </li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    position: relative;
    min-width: 0;
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
