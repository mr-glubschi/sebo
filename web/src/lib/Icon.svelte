<script lang="ts" module>
  // Einfache Strich-Symbole (24×24) für die Bedienung. Ordner-Symbole kommen aus Lucide.
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
    move: [
      "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
      "M9 13h6",
      "M13 10.5l2.5 2.5-2.5 2.5",
    ],
    merge: ["M6 4v5a4 4 0 0 0 4 4h4a4 4 0 0 0 4-4V4", "M12 13v7", "M9 17l3 3 3-3"],
    group: ["M3 4h8", "M7 8h14", "M7 12h14", "M3 16h8", "M7 20h14"],
    inbox: ["M4 13l2-8h12l2 8", "M4 13v6h16v-6h-5l-1 2h-4l-1-2z"],
    all: ["M6 4h12v16l-6-4-6 4z"],
    star: ["M12 3.5l2.6 5.3 5.9.9-4.25 4.1 1 5.85L12 16.9l-5.25 2.75 1-5.85L3.5 9.7l5.9-.9z"],
    chevron: ["M9 6l6 6-6 6"],
    cards: ["M4 4h7v7H4z", "M13 4h7v7h-7z", "M4 13h7v7H4z", "M13 13h7v7h-7z"],
    list: ["M4 5h4v4H4z", "M11 7h9", "M4 15h4v4H4z", "M11 17h9"],
    compact: ["M4 6h16", "M4 10h16", "M4 14h16", "M4 18h16"],
    search: ["M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14z", "M20 20l-4-4"],
    paste: ["M9 4h6v3H9z", "M9 5H6v15h12V5h-3"],
    copy: ["M9 9h11v11H9z", "M5 15H4V4h11v1"],
    external: ["M14 4h6v6", "M20 4L10 14", "M18 14v6H4V6h6"],
    trash: ["M4 7h16", "M10 11v6", "M14 11v6", "M6 7l1 13h10l1-13", "M9 7V4h6v3"],
    check: ["M5 12.5l4.5 4.5L19 7.5"],
    select: ["M4 4h16v16H4z", "M8 12l3 3 5-6"],
    logout: ["M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3", "M10 16l-4-4 4-4", "M6 12h10"],
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, filled = false }: { name: IconName; filled?: boolean } = $props();
</script>

<svg
  viewBox="0 0 24 24"
  width="20"
  height="20"
  fill={filled ? "currentColor" : "none"}
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
