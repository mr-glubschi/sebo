<script lang="ts">
  import { api, ApiError, errorText, type BookmarkChanges } from "./api";
  import type { Bookmark } from "./bindings/Bookmark";
  import { findUrls, normalizeUrl } from "./links";
  import FolderSelect from "./FolderSelect.svelte";
  import Icon from "./Icon.svelte";
  import { folderPath, type Tree } from "./library";
  import Modal from "./Modal.svelte";
  import { toasts } from "./toast.svelte";

  interface Props {
    tree: Tree;
    /** Alle Bookmarks, um doppelte Adressen zu erkennen. */
    bookmarks: Bookmark[];
    /** Vorbelegung für neue Bookmarks (z. B. über „Teilen“). */
    initialUrl?: string;
    initialTitle?: string;
    /** Bestehendes Bookmark bearbeiten; ohne: neues anlegen. */
    bookmark?: Bookmark;
    /** Vorauswahl für neue Bookmarks. */
    folderId?: number | null;
    onclose: () => void;
    /** Nach jeder Änderung, damit die Liste neu geladen wird. */
    onchanged: () => void;
  }

  let {
    tree,
    bookmarks,
    initialUrl,
    initialTitle,
    bookmark,
    folderId = null,
    onclose,
    onchanged,
  }: Props = $props();

  // Startwerte einmalig aus den Props übernehmen.
  const initial = (() => ({
    url: bookmark?.url ?? initialUrl ?? "",
    title: bookmark?.title ?? initialTitle ?? "",
    folder: bookmark ? bookmark.folder_id : folderId,
    editing: bookmark !== undefined,
  }))();

  let url = $state(initial.url);
  let title = $state(initial.title);
  let folder = $state<number | null>(initial.folder);
  let titleTouched = $state(initial.editing || initial.title !== "");
  let previewing = $state(false);
  let busy = $state(false);
  let error = $state("");
  let confirmDelete = $state(false);

  let previewFor = "";

  // Gewählter Ordner wurde (z. B. in einem anderen Tab) gelöscht: Unsortiert.
  $effect(() => {
    if (folder !== null && !tree.byId.has(folder)) folder = null;
  });

  // Gleiche Adresse schon gespeichert? Nur ein Hinweis, speichern bleibt möglich.
  const duplicate = $derived.by(() => {
    const raw = url.trim();
    if (raw === "") return undefined;
    const normalized = normalizeUrl(/^[a-z][a-z0-9+.-]*:/i.test(raw) ? raw : `https://${raw}`);
    if (!normalized) return undefined;
    return bookmarks.find((b) => b.url === normalized && b.id !== bookmark?.id);
  });

  const duplicatePlace = $derived(
    !duplicate
      ? ""
      : duplicate.folder_id === null
        ? "Unsortiert"
        : folderPath(tree, duplicate.folder_id)
            .map((f) => f.name)
            .join(" / "),
  );

  // Einfügen aus der Zwischenablage (nur über HTTPS oder localhost verfügbar).
  const canPaste = typeof navigator.clipboard?.readText === "function";

  async function paste() {
    try {
      const text = (await navigator.clipboard.readText()).trim();
      url = findUrls(text)[0] ?? text.slice(0, 2048);
      preview();
    } catch {
      error = "Kein Zugriff auf die Zwischenablage.";
    }
  }

  // Bewusst keine automatische Vorschau für vorbelegte Adressen (Teilen):
  // Eine fremde Seite könnte sonst über /share Abrufe durch den Server auslösen.
  // Der Titel kommt nach dem Speichern aus dem Hintergrundabruf.

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
      // In einem anderen Tab gelöscht oder geändert: Ansicht auffrischen.
      if (e instanceof ApiError && (e.status === 404 || e.status === 400)) onchanged();
      // Das Bookmark selbst gibt es nicht mehr: Dialog schließen.
      if (e instanceof ApiError && e.status === 404 && bookmark) {
        toasts.show("Das Bookmark wurde inzwischen gelöscht.", true);
        onclose();
      }
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
    <div class="field">
      <label for="bookmark-url">Adresse</label>
      <div class="with-button">
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="bookmark-url"
          autofocus
          type="text"
          inputmode="url"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          placeholder="https://…"
          maxlength="2048"
          bind:value={url}
          onblur={preview}
          onpaste={() => setTimeout(preview, 0)}
          required
        />
        {#if canPaste && !bookmark}
          <button
            type="button"
            class="secondary icon"
            aria-label="Aus Zwischenablage einfügen"
            title="Aus Zwischenablage einfügen"
            onclick={paste}
          >
            <Icon name="paste" />
          </button>
        {/if}
      </div>
      {#if duplicate}
        <p class="duplicate">Schon gespeichert in {duplicatePlace} als „{duplicate.title}“.</p>
      {/if}
    </div>

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
        <!-- svelte-ignore a11y_autofocus -->
        <button
          type="button"
          class="danger"
          disabled={busy}
          autofocus
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

  .field {
    display: grid;
    gap: 0.35rem;
    font-size: 0.9rem;
  }

  .with-button {
    display: flex;
    gap: 0.5rem;
  }

  .with-button button {
    flex: none;
    border-color: var(--border);
  }

  .duplicate {
    margin: 0;
    color: var(--muted);
    overflow-wrap: anywhere;
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
