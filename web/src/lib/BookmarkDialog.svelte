<script lang="ts">
  import { api, errorText, type BookmarkChanges } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import FolderSelect from "./FolderSelect.svelte";
  import type { Tree } from "./library";
  import Modal from "./Modal.svelte";

  interface Props {
    tree: Tree;
    /** Bestehendes Bookmark bearbeiten; ohne: neues anlegen. */
    bookmark?: Bookmark;
    /** Vorauswahl für neue Bookmarks. */
    folderId?: number | null;
    onclose: () => void;
    /** Nach jeder Änderung, damit die Liste neu geladen wird. */
    onchanged: () => void;
  }

  let { tree, bookmark, folderId = null, onclose, onchanged }: Props = $props();

  // Startwerte einmalig aus den Props übernehmen.
  const initial = (() => ({
    url: bookmark?.url ?? "",
    title: bookmark?.title ?? "",
    folder: bookmark ? bookmark.folder_id : folderId,
    editing: bookmark !== undefined,
  }))();

  let url = $state(initial.url);
  let title = $state(initial.title);
  let folder = $state<number | null>(initial.folder);
  let titleTouched = $state(initial.editing);
  let previewing = $state(false);
  let busy = $state(false);
  let error = $state("");
  let confirmDelete = $state(false);

  let previewFor = "";

  // Titelvorschlag holen, sobald die URL fertig eingegeben ist.
  async function preview() {
    const value = url.trim();
    if (bookmark || titleTouched || value === "" || value === previewFor) return;
    previewFor = value;
    previewing = true;
    try {
      const result = await api.preview(value);
      if (!titleTouched && previewFor === value && result.title) title = result.title;
    } catch {
      // Kein Vorschlag, der Titel wird später im Hintergrund geholt.
    } finally {
      if (previewFor === value) previewing = false;
    }
  }

  async function run(action: () => Promise<unknown>, close = true) {
    if (busy) return;
    busy = true;
    error = "";
    try {
      await action();
      onchanged();
      if (close) onclose();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const trimmedTitle = title.trim();
    if (!bookmark) {
      run(() =>
        api.createBookmark({
          url: url.trim(),
          title: trimmedTitle === "" ? undefined : trimmedTitle,
          folder_id: folder,
        }),
      );
      return;
    }
    const changes: BookmarkChanges = {};
    if (url.trim() !== bookmark.url) changes.url = url.trim();
    if (trimmedTitle !== bookmark.title) changes.title = trimmedTitle;
    if (folder !== bookmark.folder_id) changes.folder_id = folder;
    if (Object.keys(changes).length === 0) {
      onclose();
      return;
    }
    const id = bookmark.id;
    run(() => api.updateBookmark(id, changes));
  }
</script>

<Modal title={bookmark ? "Bookmark bearbeiten" : "Bookmark hinzufügen"} {onclose}>
  <form onsubmit={submit}>
    <label>
      Adresse
      <input
        type="text"
        inputmode="url"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        placeholder="https://…"
        bind:value={url}
        onblur={preview}
        onpaste={() => setTimeout(preview, 0)}
        required
      />
    </label>

    <label>
      Titel
      <input
        type="text"
        bind:value={title}
        oninput={() => (titleTouched = true)}
        placeholder={previewing ? "Titel wird geladen …" : "Leer lassen: Titel der Seite"}
        maxlength="300"
      />
    </label>

    <FolderSelect {tree} bind:value={folder} label="Ordner" noneLabel="Unsortiert" />

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="actions">
      <button type="submit" disabled={busy}>{bookmark ? "Speichern" : "Hinzufügen"}</button>
      <button type="button" class="secondary" onclick={onclose}>Abbrechen</button>
    </div>
  </form>

  {#if bookmark}
    {@const id = bookmark.id}
    <div class="more">
      <button
        type="button"
        class="secondary"
        disabled={busy}
        onclick={() => run(() => api.refreshBookmark(id))}
      >
        Titel und Bilder neu laden
      </button>
      {#if bookmark.image}
        <button
          type="button"
          class="secondary"
          disabled={busy}
          onclick={() => run(() => api.removeImage(id))}
        >
          Vorschaubild entfernen
        </button>
      {/if}
      {#if confirmDelete}
        <button
          type="button"
          class="danger"
          disabled={busy}
          onclick={() => run(() => api.deleteBookmark(id))}
        >
          Wirklich löschen
        </button>
      {:else}
        <button type="button" class="secondary danger-text" onclick={() => (confirmDelete = true)}>
          Löschen
        </button>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  form {
    display: grid;
    gap: 1rem;
  }

  .actions,
  .more {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .more {
    margin-top: 1.25rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border);
  }
</style>
