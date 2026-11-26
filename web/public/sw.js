// Service Worker von SEBO: hält nur die App-Hülle (Skripte, Stile, Icons) vor.
// API-Antworten und Bookmark-Bilder werden nie zwischengespeichert, es gibt keinen Offline-Modus.

const CACHE = "sebo-shell-v3";
// Nach Updates liegen alte Dateien im Cache: nur die neuesten behalten.
const MAX_ENTRIES = 30;

self.addEventListener("install", () => self.skipWaiting());

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  // Gehashte Dateien ändern sich nie: aus dem Cache, sonst laden und merken.
  if (url.pathname.startsWith("/assets/")) {
    event.respondWith(
      caches.open(CACHE).then(async (cache) => {
        const cached = await cache.match(request);
        if (cached) return cached;
        const response = await fetch(request);
        if (response.ok) {
          await cache.put(request, response.clone());
          const keys = await cache.keys();
          await Promise.all(keys.slice(0, Math.max(0, keys.length - MAX_ENTRIES)).map((key) => cache.delete(key)));
        }
        return response;
      }),
    );
  }
  // Alles andere (Seite, API, Bilder) geht direkt ans Netz.
});
