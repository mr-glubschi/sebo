<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    title: string;
    onclose: () => void;
    children: Snippet;
  }

  let { title, onclose, children }: Props = $props();

  let dialog: HTMLDialogElement;
  let pressedBackdrop = false;

  $effect(() => {
    dialog.showModal();
  });

  // Schließen per Klick auf den Hintergrund, aber nicht, wenn eine
  // Textauswahl im Dialog begonnen und außerhalb beendet wurde.
  function pointerdown(event: PointerEvent) {
    pressedBackdrop = event.target === dialog;
  }

  function click(event: MouseEvent) {
    if (pressedBackdrop && event.target === dialog) dialog.close();
    pressedBackdrop = false;
  }
</script>

<dialog bind:this={dialog} {onclose} onpointerdown={pointerdown} onclick={click}>
  <div class="inner">
    <header>
      <h2>{title}</h2>
      <button type="button" class="icon" aria-label="Schließen" onclick={() => dialog.close()}>
        <Icon name="close" />
      </button>
    </header>
    {@render children()}
  </div>
</dialog>

<style>
  dialog {
    width: min(100% - 2rem, 30rem);
    max-height: calc(100dvh - 2rem);
    padding: 0;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.25);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.45);
  }

  .inner {
    padding: 1rem 1.25rem 1.25rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.15rem;
  }

  /* Am Handy als Blatt von unten, damit die Tastatur Platz hat. */
  @media (max-width: 600px) {
    dialog {
      width: 100%;
      max-width: none;
      margin: auto 0 0;
      border-radius: var(--radius) var(--radius) 0 0;
      border-bottom: none;
    }

    .inner {
      padding-bottom: calc(1.25rem + env(safe-area-inset-bottom));
    }
  }
</style>
