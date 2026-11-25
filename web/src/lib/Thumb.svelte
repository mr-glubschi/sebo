<script lang="ts">
  import type { Bookmark } from "./bindings/Bookmark";
  import { host, hue } from "./library";

  interface Props {
    bookmark: Bookmark;
    /** `image`: Vorschaubild, sonst Favicon groß. `icon`: nur Favicon. */
    kind: "image" | "icon";
  }

  let { bookmark, kind }: Props = $props();

  // Bilder, die nicht geladen werden konnten. Reihenfolge: OG-Bild → Favicon → Buchstabe.
  let failed = $state<string[]>([]);

  const shown = $derived.by(() => {
    if (kind === "image" && bookmark.image && !failed.includes(bookmark.image)) {
      return { type: "image" as const, src: bookmark.image };
    }
    if (bookmark.icon && !failed.includes(bookmark.icon)) {
      return { type: "icon" as const, src: bookmark.icon };
    }
    return { type: "letter" as const };
  });

  const letter = $derived(
    (bookmark.title.trim() || host(bookmark.url)).charAt(0).toLocaleUpperCase("de"),
  );

  function fail(src: string) {
    failed = [...failed, src];
  }
</script>

<div class="thumb {kind}" style:--hue={hue(host(bookmark.url))}>
  {#if shown.type === "image"}
    <img class="cover" src={shown.src} alt="" loading="lazy" decoding="async" onerror={() => fail(shown.src)} />
  {:else if shown.type === "icon"}
    <img class="icon" src={shown.src} alt="" loading="lazy" decoding="async" onerror={() => fail(shown.src)} />
  {:else}
    <span class="letter" aria-hidden="true">{letter}</span>
  {/if}
</div>

<style>
  .thumb {
    display: grid;
    place-items: center;
    overflow: hidden;
    background: hsl(var(--hue) 35% 88%);
    color: hsl(var(--hue) 45% 30%);
  }

  @media (prefers-color-scheme: dark) {
    .thumb {
      background: hsl(var(--hue) 20% 22%);
      color: hsl(var(--hue) 45% 78%);
    }
  }

  .thumb.image {
    width: 100%;
    aspect-ratio: 1.91 / 1;
  }

  .thumb.icon {
    width: 100%;
    height: 100%;
    border-radius: 6px;
  }

  .cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .image .icon {
    width: 48px;
    height: 48px;
    border-radius: 8px;
  }

  .icon .icon {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  /* Favicon auf hellem Grund, viele sind für helle Seiten gemacht. */
  .thumb.icon:has(img) {
    background: #fff;
  }

  .letter {
    font-weight: 700;
    line-height: 1;
    user-select: none;
  }

  .image .letter {
    font-size: 2.5rem;
  }

  .icon .letter {
    font-size: 0.7em;
  }
</style>
