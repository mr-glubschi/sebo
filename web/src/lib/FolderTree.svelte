<script lang="ts">
  import type { Folder } from "./bindings/Folder";
  import { drag, draggable } from "./drag.svelte";
  import FolderIcon from "./FolderIcon.svelte";
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
    /** Kontextmenü: Rechtsklick oder ⋯-Knopf (dann mit `trigger`). */
    onmenu: (folder: Folder, event: MouseEvent, trigger?: HTMLElement) => void;
    /** Auswahlmodus: Klick wählt aus, statt den Ordner zu öffnen. */
    selecting: boolean;
    selected: ReadonlySet<number>;
    onselect: (id: number) => void;
  }

  let {
    tree,
    parent,
    depth,
    route,
    counts,
    collapsed,
    ontoggle,
    onnavigate,
    onmenu,
    selecting,
    selected,
    onselect,
  }: Props = $props();

  function contextmenu(folder: Folder, event: MouseEvent) {
    if (event.defaultPrevented || drag.item) return;
    onmenu(folder, event);
  }

  function click(target: Route & { kind: "folder" }, event: MouseEvent) {
    if (!selecting) {
      onnavigate(target, event);
      return;
    }
    event.preventDefault();
    onselect(target.id);
  }

  const folders = $derived(tree.children.get(parent) ?? []);
</script>

<ul class:root={depth === 0}>
  {#each folders as folder (folder.id)}
    {@const target = { kind: "folder", id: folder.id } as const}
    {@const hasChildren = (tree.children.get(folder.id)?.length ?? 0) > 0}
    {@const open = hasChildren && !collapsed.has(folder.id)}
    {@const isSelected = selecting && selected.has(folder.id)}
    <li>
      <!-- Rechtsklick ist nur eine Abkürzung, per Tastatur geht es über den ⋯-Knopf. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="row"
        class:active={route.kind === "folder" && route.id === folder.id}
        class:drop-over={drag.target === String(folder.id)}
        class:drop-before={drag.target === `before:${folder.id}`}
        class:drop-after={drag.target === `after:${folder.id}`}
        class:dragged={drag.item?.kind === "folder" && drag.item.id === folder.id}
        class:selected={isSelected}
        data-drop={String(folder.id)}
        data-reorder={open ? "top" : "both"}
        style:--depth={depth}
        use:draggable={{
          kind: "folder",
          id: folder.id,
          label: folder.name,
          icon: folder.icon,
          color: folder.color,
        }}
        oncontextmenu={(event) => contextmenu(folder, event)}
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
        <a
          href={routePath(target)}
          role={selecting ? "checkbox" : undefined}
          aria-checked={selecting ? isSelected : undefined}
          onclick={(event) => click(target, event)}
        >
          {#if selecting}
            <span class="check" class:on={isSelected} aria-hidden="true">
              <Icon name="check" />
            </span>
          {:else}
            <FolderIcon name={folder.icon} color={folder.color} />
          {/if}
          <span class="name">{folder.name}</span>
          <span class="count">{counts.get(folder.id) ?? 0}</span>
        </a>
        {#if !selecting}
          <button
            type="button"
            class="edit icon"
            aria-label="Aktionen für Ordner „{folder.name}“"
            aria-haspopup="menu"
            onclick={(event) => onmenu(folder, event, event.currentTarget)}
          >
            <Icon name="more" />
          </button>
        {/if}
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
          {onmenu}
          {selecting}
          {selected}
          {onselect}
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

  /* Einfügemarke beim Sortieren */
  .row.drop-before {
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .row.drop-after {
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .row.dragged {
    opacity: 0.4;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.active,
  .row.selected {
    background: var(--selected);
  }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    color: transparent;
    border: 2px solid var(--muted);
    border-radius: 50%;
  }

  .check :global(svg) {
    width: 14px;
    height: 14px;
    stroke-width: 3;
  }

  .check.on {
    color: var(--accent-text);
    background: var(--accent);
    border-color: var(--accent);
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
