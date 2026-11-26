<script lang="ts">
  import { api, errorText } from "./api";
  import FolderIcon from "./FolderIcon.svelte";
  import { folderPath, subtree, type Tree } from "./library";
  import Modal from "./Modal.svelte";
  import { toasts } from "./toast.svelte";

  interface Props {
    tree: Tree;
    ids: number[];
    onclose: () => void;
    onchanged: () => void;
    /** Nach dem Zusammenführen, z. B. um zum Ziel zu wechseln. */
    onmerged: (target: number, sources: number[]) => void;
  }

  let { tree, ids, onclose, onchanged, onmerged }: Props = $props();

  // Nach Tiefe, dann Pfad: Der oberste Ordner ist die naheliegende Wahl.
  const options = $derived(
    ids
      .filter((id) => tree.byId.has(id))
      .map((id) => {
        const path = folderPath(tree, id);
        // Liegt der Ordner in einem anderen ausgewählten, kann er nicht Ziel sein.
        const inside = ids.some((other) => other !== id && subtree(tree, other).has(id));
        return { id, path, name: path.map((f) => f.name).join(" / "), inside };
      })
      .sort((a, b) => a.path.length - b.path.length || a.name.localeCompare(b.name, "de")),
  );

  let target = $state<number | null>(null);
  $effect(() => {
    if (target === null || !options.some((o) => o.id === target && !o.inside)) {
      target = options.find((o) => !o.inside)?.id ?? null;
    }
  });

  let busy = $state(false);
  let error = $state("");

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy || target === null) return;
    const into = target;
    const sources = options.map((o) => o.id).filter((id) => id !== into);
    const name = tree.byId.get(into)?.name ?? "";
    busy = true;
    error = "";
    try {
      await api.mergeFolders(into, sources);
      toasts.show(`${sources.length + 1} Ordner zusammengeführt in „${name}“`);
      onmerged(into, sources);
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

<Modal title="{ids.length} Ordner zusammenführen" {onclose}>
  <form onsubmit={submit}>
    <p>
      Bookmarks und Unterordner landen im gewählten Ordner, gleichnamige Unterordner werden
      ebenfalls zusammengeführt. Die anderen Ordner werden danach gelöscht.
    </p>
    <fieldset>
      <legend>Zusammenführen in</legend>
      {#each options as option (option.id)}
        <label class="choice" class:disabled={option.inside}>
          <input type="radio" bind:group={target} value={option.id} disabled={option.inside} />
          <FolderIcon name={option.path.at(-1)?.icon} />
          <span>{option.name}</span>
        </label>
      {/each}
    </fieldset>
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <div class="actions">
      <button type="submit" disabled={busy || target === null}>Zusammenführen</button>
      <button type="button" class="secondary" onclick={onclose}>Abbrechen</button>
    </div>
  </form>
</Modal>

<style>
  form {
    display: grid;
    gap: 1rem;
  }

  p {
    margin: 0;
  }

  fieldset {
    display: grid;
    gap: 0.25rem;
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

  .choice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0;
    font-size: 1rem;
  }

  .choice span {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .choice.disabled {
    color: var(--muted);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
