<script lang="ts">
  import type { DeleteContents } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import { subtree, type Tree } from "./library";

  interface Props {
    tree: Tree;
    bookmarks: Bookmark[];
    /** Zu löschende Ordner. */
    ids: number[];
    contents: DeleteContents;
    /** Ob die Ordner etwas enthalten (dann braucht es eine Wahl). */
    empty?: boolean;
  }

  let { tree, bookmarks, ids, contents = $bindable(), empty = $bindable() }: Props = $props();

  const selected = $derived(new Set(ids));
  const all = $derived.by(() => {
    const result = new Set<number>();
    for (const id of ids) for (const folder of subtree(tree, id)) result.add(folder);
    return result;
  });
  const content = $derived({
    folders: [...all].filter((id) => !selected.has(id)).length,
    bookmarks: bookmarks.filter((b) => b.folder_id !== null && all.has(b.folder_id)).length,
  });

  $effect(() => {
    empty = content.folders === 0 && content.bookmarks === 0;
  });

  function plural(count: number, one: string, many: string) {
    return `${count} ${count === 1 ? one : many}`;
  }
</script>

{#if content.folders > 0 || content.bookmarks > 0}
  <div class="choice-group">
    <p>
      {ids.length === 1 ? "Der Ordner enthält" : "Die Ordner enthalten"}
      {[
        content.bookmarks > 0 ? plural(content.bookmarks, "Bookmark", "Bookmarks") : "",
        content.folders > 0 ? plural(content.folders, "Unterordner", "Unterordner") : "",
      ]
        .filter(Boolean)
        .join(" und ")}.
    </p>
    <label class="choice">
      <input type="radio" bind:group={contents} value="move" />
      Inhalt eine Ebene nach oben verschieben
    </label>
    <label class="choice">
      <input type="radio" bind:group={contents} value="delete" />
      Inhalt mitlöschen
    </label>
  </div>
{/if}

<style>
  .choice-group {
    display: grid;
    gap: 0.75rem;
  }

  p {
    margin: 0;
  }

  .choice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
</style>
