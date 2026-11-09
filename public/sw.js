// Address Explorer PWA service worker.
//
// There is no build-time precache manifest. Stable shell/worker/seed paths are
// cached on install; hashed Dioxus assets are registered by the app after boot.

const CACHE_NAME = 'address-explorer-v1';
// Worker and registration files keep stable URLs across deployments. Fetch
// them network-first (revalidating the HTTP cache) so a new app never pairs
// with a stale worker; fall back to the cache when offline.
const REVALIDATE_PATHS = new Set(['/local-worker.js', '/sw-register.js']);
const REVALIDATE_PREFIXES = ['/worker/'];
const STABLE_PRECACHE = [
  '/',
  '/index.html',
  '/manifest.webmanifest',
  '/icons/icon-192.png',
  '/icons/icon-512.png',
  '/icons/maskable-512.png',
  '/icons/apple-touch-icon.png',
  '/local-worker.js',
  '/worker/address_explorer_web_worker.js',
  '/worker/address_explorer_web_worker_bg.wasm',
  '/btc_usd.bin',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE_NAME);
      await Promise.all(STABLE_PRECACHE.map((url) => cache.add(url).catch(() => {})));
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== CACHE_NAME).map((key) => caches.delete(key))),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('message', (event) => {
  const data = event.data;
  if (!data || data.type !== 'cache-urls' || !Array.isArray(data.urls)) {
    return;
  }
  const origin = self.location.origin;
  const urls = data.urls.filter((url) => {
    try {
      const parsed = new URL(url, origin);
      return parsed.origin === origin && !parsed.pathname.startsWith('/api/');
    } catch {
      return false;
    }
  });
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) =>
      Promise.all(urls.map((url) => cache.add(url).catch(() => {}))),
    ),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  if (url.pathname.startsWith('/api/')) return;

  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request)
        .then((response) => {
          const copy = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put('/', copy)).catch(() => {});
          return response;
        })
        .catch(() => caches.match('/')),
    );
    return;
  }

  const revalidate =
    REVALIDATE_PATHS.has(url.pathname) ||
    REVALIDATE_PREFIXES.some((prefix) => url.pathname.startsWith(prefix));
  if (revalidate) {
    event.respondWith(
      fetch(request, { cache: 'no-cache' })
        .then((response) => {
          if (response.ok) {
            const copy = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(request, copy)).catch(() => {});
          }
          return response;
        })
        .catch(() => caches.match(request)),
    );
    return;
  }

  event.respondWith(
    caches.match(request).then((cached) => {
      if (cached) return cached;
      return fetch(request).then((response) => {
        if (response.ok) {
          const copy = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(request, copy)).catch(() => {});
        }
        return response;
      });
    }),
  );
});
