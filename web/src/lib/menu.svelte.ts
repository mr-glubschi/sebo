// Kontextmenü: per Rechtsklick (Maus, Kontextmenü-Taste) oder ⋯-Knopf (Touch).
// Es gibt nur ein Menü gleichzeitig, angezeigt von <ContextMenu> in Home.

import type { IconName } from "./Icon.svelte";

export interface MenuItem {
  label: string;
  icon: IconName;
  /** Gefülltes Symbol (z. B. Stern bei Favoriten). */
  filled?: boolean;
  danger?: boolean;
  action: () => void;
}

export interface OpenMenu {
  x: number;
  y: number;
  items: MenuItem[];
  /** Bekommt nach dem Schließen den Fokus zurück. */
  trigger: HTMLElement | null;
}

class MenuState {
  open = $state<OpenMenu | null>(null);

  /**
   * Öffnet das Menü an der Zeigerposition oder, ohne Position (Tastatur,
   * Knopf), unter dem auslösenden Element.
   */
  show(event: MouseEvent, items: MenuItem[], trigger?: HTMLElement) {
    event.preventDefault();
    event.stopPropagation();
    const element = trigger ?? (event.currentTarget as HTMLElement | null);
    let x = event.clientX;
    let y = event.clientY;
    // Tastatur oder Knopf: keine sinnvolle Zeigerposition.
    if (trigger || (x === 0 && y === 0)) {
      const rect = element?.getBoundingClientRect();
      if (rect) {
        x = trigger ? rect.right : rect.left + 16;
        y = trigger ? rect.bottom : rect.top + rect.height / 2;
      }
    }
    this.open = { x, y, items, trigger: element };
  }

  close(restoreFocus = false) {
    const trigger = this.open?.trigger;
    this.open = null;
    if (restoreFocus && trigger?.isConnected) trigger.focus();
  }
}

export const menu = new MenuState();
