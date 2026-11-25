<script lang="ts">
  import { api, ApiError, errorText, type DeleteContents, type FolderChanges } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import type { Folder } from "./bindings/Folder";
  import FolderSelect from "./FolderSelect.svelte";
  import { subtree, type Tree } from "./library";
  import Modal from "./Modal.svelte";

  interface Props {
    tree: Tree;
    bookmarks: Bookmark[];
    /** Bestehenden Ordner bearbeiten; ohne: neuen anlegen. */
    folder?: Folder;
    /** Vorauswahl für neue Ordner. */
    parentId?: number | null;
    onclose: () => void;
    onchanged: () => void;
    /** Ordner wurde gelöscht. */
    ondeleted: (folder: Folder) => void;
  }

  let { tree, bookmarks, folder, parentId = null, onclose, onchanged, ondeleted }: Props =
    $props();

  const initial = (() => ({
    name: folder?.name ?? "",
    parent: folder ? folder.parent_id : parentId,
  }))();

  let name = $state(initial.name);
  let parent = $state<number | null>(initial.parent);
  let contents = $state<DeleteContents>("move");
  let confirmDelete = $state(false);
  let busy = $state(false);
  let error = $state("");

  // Ein Ordner darf nicht in sich selbst oder einen seiner Unterordner.
  const own = $derived(folder ? subtree(tree, folder.id) : new Set<number>());
  const content = $derived({
    folders: own.size - 1,
    bookmarks: bookmarks.filter((b) => b.folder_id !== null && own.has(b.folder_id)).length,
  });
  const isEmpty = $derived(content.folders === 0 && content.bookmarks === 0);

  async function run(action: () => Promise<unknown>) {
    if (busy) return false;
    busy = true;
    error = "";
    try {
      await action();
      return true;
    } catch (e) {
      error = errorText(e);
      if (e instanceof ApiError && (e.status === 404 || e.status === 400)) onchanged();
      return false;
    } finally {
      busy = false;
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const trimmed = name.trim();
    let ok: boolean;
    if (!folder) {
      ok = await run(() => api.createFolder(trimmed, parent));
    } else {
      const changes: FolderChanges = {};
      if (trimmed !== folder.name) changes.name = trimmed;
      if (parent !== folder.parent_id) changes.parent_id = parent;
      const id = folder.id;
      ok = Object.keys(changes).length === 0 || (await run(() => api.updateFolder(id, changes)));
    }
    if (ok) {
      onchanged();
      onclose();
    }
  }

  async function remove() {
    if (!folder) return;
    const deleted = folder;
    if (await run(() => api.deleteFolder(deleted.id, isEmpty ? "delete" : contents))) {
      onchanged();
      ondeleted(deleted);
      onclose();
    }
  }

  function plural(count: number, one: string, many: string) {
    return `${count} ${count === 1 ? one : many}`;
  }
</script>

<Modal title={folder ? "Ordner bearbeiten" : "Neuer Ordner"} {onclose}>
  <form onsubmit={submit}>
    <label>
      Name
      <!-- svelte-ignore a11y_autofocus -->
      <input type="text" bind:value={name} maxlength="200" required autofocus />
    </label>

    <FolderSelect
      {tree}
      bind:value={parent}
      exclude={own}
      label="Liegt in"
      noneLabel="Oberste Ebene"
    />

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="actions">
      <button type="submit" disabled={busy}>{folder ? "Speichern" : "Anlegen"}</button>
      <button type="button" class="secondary" onclick={onclose}>Abbrechen</button>
    </div>
  </form>

  {#if folder}
    <div class="delete">
      {#if !confirmDelete}
        <button type="button" class="secondary danger-text" onclick={() => (confirmDelete = true)}>
          Ordner löschen
        </button>
      {:else}
        {#if !isEmpty}
          <p>
            Der Ordner enthält
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
        {/if}
        <div class="actions">
          <!-- svelte-ignore a11y_autofocus -->
          <button type="button" class="danger" disabled={busy} onclick={remove} autofocus>
            Wirklich löschen
          </button>
          <button type="button" class="secondary" onclick={() => (confirmDelete = false)}>
            Nicht löschen
          </button>
        </div>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  form,
  .delete {
    display: grid;
    gap: 1rem;
  }

  .delete {
    margin-top: 1.25rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border);
    justify-items: start;
  }

  .delete p {
    margin: 0;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .choice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
</style>
