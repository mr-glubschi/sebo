import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) throw new Error("Element #app fehlt");

export default mount(App, { target });

// App-Hülle für die installierte Web-App vorhalten (nicht im Entwicklungsmodus).
if ("serviceWorker" in navigator && import.meta.env.PROD) {
  navigator.serviceWorker.register("/sw.js").catch(() => {
    // Ohne Service Worker funktioniert SEBO genauso, nur ohne Installation.
  });
}
