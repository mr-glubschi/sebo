<script lang="ts">
  import { api, ApiError, errorText, type DeleteContents, type FolderChanges } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import type { Folder } from "./bindings/Folder";
  import DeleteFolderChoice from "./DeleteFolderChoice.svelte";
  import FolderIcon from "./FolderIcon.svelte";
  import { FOLDER_ICON_NAMES, FOLDER_ICONS } from "./folderIcons";
  import FolderSelect from "./FolderSelect.svelte";
  import { subtree, type Tree } from "./library";
  import Modal from "./Modal.svelte";
  import { toasts } from "./toast.svelte";

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
    icon: folder?.icon ?? null,
  }))();

  let name = $state(initial.name);
  let parent = $state<number | null>(initial.parent);
  let icon = $state<string | null>(initial.icon);
  let contents = $state<DeleteContents>("move");
  let isEmpty = $state(true);

  // Gewählter Elternordner wurde anderswo gelöscht: oberste Ebene.
  $effect(() => {
    if (parent !== null && !tree.byId.has(parent)) parent = null;
  });
  let confirmDelete = $state(false);
  let busy = $state(false);
  let error = $state("");

  // Ein Ordner darf nicht in sich selbst oder einen seiner Unterordner.
  const own = $derived(folder ? subtree(tree, folder.id) : new Set<number>());

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
      if (e instanceof ApiError && e.status === 404 && folder) {
        toasts.show("Der Ordner wurde inzwischen gelöscht.", true);
        onclose();
      }
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
      ok = await run(async () => {
        const created = await api.createFolder(trimmed, parent);
        if (icon !== null) await api.updateFolder(created.id, { icon });
      });
    } else {
      const changes: FolderChanges = {};
      if (trimmed !== folder.name) changes.name = trimmed;
      if (parent !== folder.parent_id) changes.parent_id = parent;
      if (icon !== folder.icon) changes.icon = icon;
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

    <fieldset>
      <legend>Symbol</legend>
      <div class="icons">
        <label class="icon-option" title="Standard">
          <input type="radio" bind:group={icon} value={null} aria-label="Standard" />
          <FolderIcon name={null} />
        </label>
        {#each FOLDER_ICON_NAMES as option (option)}
          {@const label = FOLDER_ICONS[option]?.label ?? option}
          <label class="icon-option" title={label}>
            <input type="radio" bind:group={icon} value={option} aria-label={label} />
            <FolderIcon name={option} />
          </label>
        {/each}
      </div>
    </fieldset>

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
        <DeleteFolderChoice
          {tree}
          {bookmarks}
          ids={[folder.id]}
          bind:contents
          bind:empty={isEmpty}
        />
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

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  fieldset {
    min-width: 0;
    margin: 0;
    padding: 0;
    border: none;
  }

  legend {
    margin-bottom: 0.35rem;
    padding: 0;
    font-size: 0.9rem;
  }

  .icons {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(2.5rem, 1fr));
    gap: 0.25rem;
    max-height: 11rem;
    overflow-y: auto;
    padding: 0.25rem;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) - 4px);
  }

  .icon-option {
    position: relative;
    display: grid;
    place-items: center;
    height: 2.5rem;
    color: var(--muted);
    border-radius: calc(var(--radius) - 5px);
    cursor: pointer;
  }

  .icon-option:hover {
    background: var(--hover);
  }

  .icon-option:has(input:checked) {
    color: var(--text);
    background: var(--selected);
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .icon-option:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  /* Radio unsichtbar, aber per Tastatur erreichbar */
  .icon-option input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
</style>
