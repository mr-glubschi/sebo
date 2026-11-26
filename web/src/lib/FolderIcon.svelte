<script lang="ts">
  import { FOLDER_ICONS } from "./folderIcons";
  import Icon from "./Icon.svelte";

  // Unbekannter oder fehlender Name: normales Ordner-Symbol.
  let { name }: { name: string | null | undefined } = $props();

  const node = $derived(name && Object.hasOwn(FOLDER_ICONS, name) ? FOLDER_ICONS[name]?.node : undefined);
</script>

{#if node}
  <svg
    viewBox="0 0 24 24"
    width="20"
    height="20"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    {#each node as [tag, attrs], index (index)}
      <svelte:element this={tag} {...attrs} />
    {/each}
  </svg>
{:else}
  <Icon name="folder" />
{/if}

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
