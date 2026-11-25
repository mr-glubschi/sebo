// Kurze Hinweise unten am Bildschirm.

export interface Toast {
  id: number;
  text: string;
  error: boolean;
}

let next = 1;

class Toasts {
  list = $state<Toast[]>([]);

  show(text: string, error = false) {
    const id = next++;
    this.list = [...this.list.slice(-2), { id, text, error }];
    setTimeout(() => this.dismiss(id), error ? 6000 : 3500);
  }

  dismiss(id: number) {
    this.list = this.list.filter((toast) => toast.id !== id);
  }
}

export const toasts = new Toasts();
