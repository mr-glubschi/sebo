<script lang="ts">
  import Modal from "./Modal.svelte";

  interface Props {
    title: string;
    message: string;
    confirmLabel: string;
    /** Gibt `false` zurück, wenn der Dialog offen bleiben soll. */
    onconfirm: () => Promise<boolean>;
    onclose: () => void;
  }

  let { title, message, confirmLabel, onconfirm, onclose }: Props = $props();

  let busy = $state(false);

  async function confirm() {
    if (busy) return;
    busy = true;
    try {
      if (await onconfirm()) onclose();
    } finally {
      busy = false;
    }
  }
</script>

<Modal {title} {onclose}>
  <p>{message}</p>
  <div class="actions">
    <!-- svelte-ignore a11y_autofocus -->
    <button type="button" class="danger" disabled={busy} onclick={confirm} autofocus>
      {confirmLabel}
    </button>
    <button type="button" class="secondary" onclick={onclose}>Abbrechen</button>
  </div>
</Modal>

<style>
  p {
    margin: 0 0 1.25rem;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
