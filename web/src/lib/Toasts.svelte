<script lang="ts">
  import { toasts } from "./toast.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts.list as toast (toast.id)}
    <button type="button" class="toast" class:error={toast.error} onclick={() => toasts.dismiss(toast.id)}>
      {toast.text}
    </button>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: calc(1rem + env(safe-area-inset-bottom));
    z-index: 50;
    display: grid;
    gap: 0.5rem;
    width: min(100% - 2rem, 26rem);
    transform: translateX(-50%);
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    padding: 0.65rem 1rem;
    font-weight: 500;
    text-align: left;
    color: var(--bg);
    background: var(--text);
    border: none;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.25);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .toast.error {
    color: #fff;
    background: var(--danger-strong);
  }
</style>
