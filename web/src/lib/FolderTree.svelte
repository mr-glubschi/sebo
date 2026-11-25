<script lang="ts">
  import type { Folder } from "./bindings/Folder";
  import { drag, draggable } from "./drag.svelte";
  import FolderTree from "./FolderTree.svelte";
  import Icon from "./Icon.svelte";
  import { routePath, type Route, type Tree } from "./library";

  interface Props {
    tree: Tree;
    parent: number | null;
    depth: number;
    route: Route;
    counts: Map<number, number>;
    /** Zugeklappte Ordner. */
    collapsed: Set<number>;
    ontoggle: (id: number) => void;
    onnavigate: (route: Route, event: MouseEvent) => void;
    onedit: (folder: Folder) => void;
  }

  let { tree, parent, depth, route, counts, collapsed, ontoggle, onnavigate, onedit }: Props =
    $props();

  const folders = $derived(tree.children.get(parent) ?? []);
</script>

<ul class:root={depth === 0}>
  {#each folders as folder (folder.id)}
    {@const target = { kind: "folder", id: folder.id } as const}
    {@const hasChildren = (tree.children.get(folder.id)?.length ?? 0) > 0}
    {@const open = hasChildren && !collapsed.has(folder.id)}
    <li>
      <div
        class="row"
        class:active={route.kind === "folder" && route.id === folder.id}
        class:drop-over={drag.target === String(folder.id)}
        class:dragged={drag.item?.kind === "folder" && drag.item.id === folder.id}
        data-drop={String(folder.id)}
        style:--depth={depth}
        use:draggable={{ kind: "folder", id: folder.id, label: folder.name }}
      >
        {#if hasChildren}
          <button
            type="button"
            class="toggle icon"
            class:open
            aria-label={open ? `${folder.name} zuklappen` : `${folder.name} aufklappen`}
            aria-expanded={open}
            onclick={() => ontoggle(folder.id)}
          >
            <Icon name="chevron" />
          </button>
        {:else}
          <span class="toggle"></span>
        {/if}
        <a href={routePath(target)} onclick={(event) => onnavigate(target, event)}>
          <Icon name="folder" />
          <span class="name">{folder.name}</span>
          <span class="count">{counts.get(folder.id) ?? 0}</span>
        </a>
        <button
          type="button"
          class="edit icon"
          aria-label="Ordner „{folder.name}“ bearbeiten"
          onclick={() => onedit(folder)}
        >
          <Icon name="edit" />
        </button>
      </div>
      {#if open}
        <FolderTree
          {tree}
          parent={folder.id}
          depth={depth + 1}
          {route}
          {counts}
          {collapsed}
          {ontoggle}
          {onnavigate}
          {onedit}
        />
      {/if}
    </li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .row {
    display: flex;
    align-items: center;
    padding-left: calc(var(--depth) * 0.9rem);
    border-radius: calc(var(--radius) - 4px);
    -webkit-touch-callout: none;
    user-select: none;
  }

  .row.dragged {
    opacity: 0.4;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.active {
    background: var(--selected);
  }

  .toggle {
    flex: none;
    width: 1.75rem;
    height: 1.75rem;
    padding: 0.2rem;
    color: var(--muted);
  }

  .toggle :global(svg) {
    width: 16px;
    height: 16px;
    margin: auto;
    transition: transform 0.15s;
  }

  /* Am Handy größere Ziele */
  @media (pointer: coarse) {
    .toggle {
      width: 2.25rem;
      height: 2.25rem;
    }
  }

  .toggle.open :global(svg) {
    transform: rotate(90deg);
  }

  a {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
    padding: 0.45rem 0.25rem;
    color: inherit;
    text-decoration: none;
  }

  .name {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .edit {
    padding: 0.3rem;
    color: var(--muted);
  }

  @media (hover: hover) {
    .edit {
      opacity: 0;
    }

    .row:hover .edit,
    .row:focus-within .edit {
      opacity: 1;
    }
  }
</style>
