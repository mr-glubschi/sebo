<script lang="ts" module>
  // Einfache Strich-Symbole (24×24), keine Symbol-Bibliothek nötig.
  const PATHS = {
    plus: ["M12 5v14", "M5 12h14"],
    menu: ["M4 6h16", "M4 12h16", "M4 18h16"],
    close: ["M6 6l12 12", "M18 6L6 18"],
    edit: ["M4 20h4L19 9l-4-4L4 16v4z", "M14 6l4 4"],
    more: ["M5 12h.01", "M12 12h.01", "M19 12h.01"],
    folder: ["M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"],
    folderPlus: [
      "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
      "M12 10v6",
      "M9 13h6",
    ],
    inbox: ["M4 13l2-8h12l2 8", "M4 13v6h16v-6h-5l-1 2h-4l-1-2z"],
    all: ["M6 4h12v16l-6-4-6 4z"],
    chevron: ["M9 6l6 6-6 6"],
    cards: ["M4 4h7v7H4z", "M13 4h7v7h-7z", "M4 13h7v7H4z", "M13 13h7v7h-7z"],
    list: ["M4 5h4v4H4z", "M11 7h9", "M4 15h4v4H4z", "M11 17h9"],
    compact: ["M4 6h16", "M4 10h16", "M4 14h16", "M4 18h16"],
    logout: ["M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3", "M10 16l-4-4 4-4", "M6 12h10"],
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

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
  {#each PATHS[name] as d (d)}
    <path {d} />
  {/each}
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
