<script lang="ts">
  import { folderColor } from "./folderColors";
  import { FOLDER_ICONS } from "./folderIcons";
  import Icon from "./Icon.svelte";

  // Unbekannter oder fehlender Name: normales Ordner-Symbol; ohne Farbe: Akzentfarbe.
  let { name, color }: { name: string | null | undefined; color?: string | null } = $props();

  const node = $derived(name && Object.hasOwn(FOLDER_ICONS, name) ? FOLDER_ICONS[name]?.node : undefined);
</script>

<span class="folder-icon" style:color={folderColor(color)}>
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
</span>

<style>
  .folder-icon {
    flex: none;
    display: grid;
  }

  svg {
    display: block;
  }
</style>
