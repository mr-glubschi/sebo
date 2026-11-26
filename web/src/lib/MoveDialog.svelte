<script lang="ts">
  import { api, errorText } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import FolderSelect from "./FolderSelect.svelte";
  import { subtree, type Tree } from "./library";
  import Modal from "./Modal.svelte";
  import { toasts } from "./toast.svelte";

  interface Props {
    tree: Tree;
    bookmarks: Bookmark[];
    /** Was verschoben wird. */
    kind: "bookmark" | "folder";
    ids: number[];
    onclose: () => void;
    onchanged: () => void;
  }

  let { tree, bookmarks, kind, ids, onclose, onchanged }: Props = $props();

  // Vorauswahl: der gemeinsame Ort, falls alle am selben liegen.
  const initial = (() => {
    const places = new Set(
      kind === "bookmark"
        ? bookmarks.filter((b) => ids.includes(b.id)).map((b) => b.folder_id)
        : ids.map((id) => tree.byId.get(id)?.parent_id ?? null),
    );
    return places.size === 1 ? ([...places][0] ?? null) : null;
  })();

  let target = $state<number | null>(initial);
  let busy = $state(false);
  let error = $state("");

  // Ordner nicht in sich selbst oder einen ihrer Unterordner.
  const exclude = $derived.by(() => {
    const result = new Set<number>();
    if (kind === "folder") {
      for (const id of ids) for (const folder of subtree(tree, id)) result.add(folder);
    }
    return result;
  });

  const title = $derived(
    ids.length === 1
      ? kind === "bookmark"
        ? "Bookmark verschieben"
        : "Ordner verschieben"
      : `${ids.length} ${kind === "bookmark" ? "Bookmarks" : "Ordner"} verschieben`,
  );

  function targetName() {
    if (target === null) return kind === "bookmark" ? "Unsortiert" : "oberste Ebene";
    return tree.byId.get(target)?.name ?? "?";
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    busy = true;
    error = "";
    try {
      if (kind === "bookmark") {
        const [only] = ids;
        if (ids.length === 1 && only !== undefined) {
          await api.updateBookmark(only, { folder_id: target });
        } else {
          await api.bulkBookmarks(ids, { action: "move", folder_id: target });
        }
      } else {
        for (const id of ids) {
          if ((tree.byId.get(id)?.parent_id ?? null) !== target) {
            await api.updateFolder(id, { parent_id: target });
          }
        }
      }
      toasts.show(
        `${ids.length === 1 ? "Verschoben" : `${ids.length} verschoben`} → ${targetName()}`,
      );
      onchanged();
      onclose();
    } catch (e) {
      error = errorText(e);
      onchanged();
    } finally {
      busy = false;
    }
  }
</script>

<Modal {title} {onclose}>
  <form onsubmit={submit}>
    <FolderSelect
      {tree}
      bind:value={target}
      {exclude}
      label="Nach"
      noneLabel={kind === "bookmark" ? "Unsortiert" : "Oberste Ebene"}
    />
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <div class="actions">
      <!-- svelte-ignore a11y_autofocus -->
      <button type="submit" disabled={busy} autofocus>Verschieben</button>
      <button type="button" class="secondary" onclick={onclose}>Abbrechen</button>
    </div>
  </form>
</Modal>

<style>
  form {
    display: grid;
    gap: 1rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
