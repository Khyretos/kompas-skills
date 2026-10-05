// Service worker for Kreative Kompanion: caches the shell on install and serves assets offline, while letting API streams and downloads hit the network.
const CACHE = "kk-shell-v1";
const SHELL = ["./", "index.html", "main.js", "styles.css", "manifest.webmanifest", "icon-192.png", "icon-512.png"];

self.addEventListener("install", (e) => {
  e.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(SHELL)).then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (e) => {
  const currentCaches = caches.keys().then((names) => names.filter((name) => name !== CACHE));
  e.waitUntil(
    currentCaches.then((cachesToDelete) => Promise.all(cachesToDelete.map((cache) => caches.delete(cache)))).then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  // The network only: other hosts, the API (live data, event stream) and downloads.
  if (url.origin !== self.location.origin || url.pathname.startsWith("/api/") || url.pathname.startsWith("/download/")
    || (req.headers.get("Accept") || "").includes("text/event-stream")) return;
  // Pages: the network first, a cached copy of the app when offline.
  // Everything else (scripts, styles, icons): the network first, the cache when offline.
  const key = req.mode === "navigate" ? "index.html" : req;
  e.respondWith((async () => {
    try {
      const res = await fetch(req);
      if (res.ok) {
        const copy = res.clone();
        e.waitUntil(caches.open(CACHE).then((c) => c.put(key, copy)));
      }
      return res;
    } catch (err) {
      const cached = await caches.match(key);
      if (cached) return cached;
      throw err;
    }
  })());
});
