<script lang="ts">
  import { api, errorText, type DeleteContents } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import DeleteFolderChoice from "./DeleteFolderChoice.svelte";
  import type { Tree } from "./library";
  import Modal from "./Modal.svelte";
  import { toasts } from "./toast.svelte";

  interface Props {
    tree: Tree;
    bookmarks: Bookmark[];
    ids: number[];
    onclose: () => void;
    onchanged: () => void;
    ondeleted: (ids: number[]) => void;
  }

  let { tree, bookmarks, ids, onclose, onchanged, ondeleted }: Props = $props();

  let contents = $state<DeleteContents>("move");
  let empty = $state(true);
  let busy = $state(false);
  let error = $state("");

  const names = $derived(
    ids.flatMap((id) => {
      const name = tree.byId.get(id)?.name;
      return name === undefined ? [] : [`„${name}“`];
    }),
  );
  const shown = $derived(
    names.length <= 5
      ? names.join(", ")
      : `${names.slice(0, 5).join(", ")} und ${names.length - 5} weitere`,
  );

  async function remove() {
    if (busy) return;
    busy = true;
    error = "";
    try {
      const mode = empty ? "delete" : contents;
      const [only] = ids;
      if (ids.length === 1 && only !== undefined) await api.deleteFolder(only, mode);
      else await api.deleteFolders(ids, mode);
      toasts.show(ids.length === 1 ? "Ordner gelöscht" : `${ids.length} Ordner gelöscht`);
      ondeleted(ids);
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

<Modal title={ids.length === 1 ? "Ordner löschen" : `${ids.length} Ordner löschen`} {onclose}>
  <div class="body">
    <p class="names">{shown}</p>
    <DeleteFolderChoice {tree} {bookmarks} {ids} bind:contents bind:empty />
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <div class="actions">
      <!-- svelte-ignore a11y_autofocus -->
      <button type="button" class="danger" disabled={busy} onclick={remove} autofocus>
        Wirklich löschen
      </button>
      <button type="button" class="secondary" onclick={onclose}>Abbrechen</button>
    </div>
  </div>
</Modal>

<style>
  .body {
    display: grid;
    gap: 1rem;
  }

  .names {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
