<script lang="ts">
  import { menu, type MenuItem } from "./menu.svelte";
  import Icon from "./Icon.svelte";

  // Mit Maus am Zeiger, am Handy als Liste am unteren Rand.
  const SHEET_QUERY = "(max-width: 600px), (pointer: coarse)";

  let element = $state<HTMLElement>();
  let left = $state(0);
  let top = $state(0);
  let sheet = $state(false);

  $effect(() => {
    const open = menu.open;
    if (!open) return;
    sheet = matchMedia(SHEET_QUERY).matches;
    left = open.x;
    top = open.y;
  });

  // Nach dem Zeichnen: im Fenster halten und das erste Element fokussieren.
  $effect(() => {
    const open = menu.open;
    if (!open || !element) return;
    if (!sheet) {
      const rect = element.getBoundingClientRect();
      left = Math.max(8, Math.min(open.x, innerWidth - rect.width - 8));
      top =
        open.y + rect.height > innerHeight - 8 ? Math.max(8, open.y - rect.height) : open.y;
    }
    items()[0]?.focus();
  });

  // Klick daneben, Scrollen, Größenänderung oder Fensterwechsel schließen das Menü.
  $effect(() => {
    if (!menu.open) return;
    const outside = (event: Event) => {
      if (!element?.contains(event.target as Node)) menu.close();
    };
    const close = () => menu.close();
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("scroll", outside, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("scroll", outside, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  });

  function items(): HTMLElement[] {
    return Array.from(element?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? []);
  }

  function keydown(event: KeyboardEvent) {
    const list = items();
    const index = list.indexOf(document.activeElement as HTMLElement);
    let next: number | undefined;
    switch (event.key) {
      case "ArrowDown":
        next = (index + 1) % list.length;
        break;
      case "ArrowUp":
        next = (index - 1 + list.length) % list.length;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = list.length - 1;
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        menu.close(true);
        return;
      case "Tab":
        event.preventDefault();
        menu.close(true);
        return;
      default:
        return;
    }
    event.preventDefault();
    list[next]?.focus();
  }

  function choose(item: MenuItem) {
    // Erst schließen (Fokus zurück), dann ausführen: Ein geöffneter Dialog
    // setzt seinen eigenen Fokus.
    menu.close(true);
    item.action();
  }
</script>

{#if menu.open}
  {#if sheet}
    <div class="backdrop" aria-hidden="true"></div>
  {/if}
  <div
    bind:this={element}
    class="menu"
    class:sheet
    role="menu"
    tabindex="-1"
    style:left={sheet ? undefined : `${left}px`}
    style:top={sheet ? undefined : `${top}px`}
    onkeydown={keydown}
    oncontextmenu={(event) => event.preventDefault()}
  >
    {#each menu.open.items as item (item.label)}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        class:destructive={item.danger}
        onclick={() => choose(item)}
      >
        <Icon name={item.icon} filled={item.filled} />
        <span>{item.label}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 70;
    display: grid;
    min-width: 13rem;
    max-width: calc(100vw - 1rem);
    padding: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) - 2px);
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.22);
  }

  .menu:focus {
    outline: none;
  }

  button {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.5rem 0.65rem;
    font-weight: 500;
    text-align: left;
    color: var(--text);
    background: transparent;
    border: none;
    border-radius: calc(var(--radius) - 5px);
  }

  button :global(svg) {
    color: var(--muted);
  }

  button:hover,
  button:focus-visible {
    background: var(--hover);
    outline: none;
  }

  button.destructive,
  button.destructive :global(svg) {
    color: var(--danger);
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 69;
    background: rgb(0 0 0 / 0.4);
  }

  /* Handy: Blatt von unten mit großen Zielen */
  .sheet {
    inset: auto 0 0;
    max-width: none;
    padding: 0.5rem 0.5rem calc(0.5rem + env(safe-area-inset-bottom));
    border-bottom: none;
    border-radius: var(--radius) var(--radius) 0 0;
  }

  .sheet button {
    padding: 0.8rem 0.75rem;
    font-size: 1rem;
  }
</style>
