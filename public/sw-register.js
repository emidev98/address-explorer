// Register the PWA service worker and report loaded hashed assets so the
// worker can cache them without a build-time precache manifest.
if ('serviceWorker' in navigator) {
  const reported = new Set();

  const post = (registration, urls) => {
    if (!registration.active) return;
    const fresh = urls.filter((url) => {
      if (!url.startsWith(location.origin) || reported.has(url)) return false;
      reported.add(url);
      return true;
    });
    if (fresh.length) {
      registration.active.postMessage({ type: 'cache-urls', urls: fresh });
    }
  };

  const collect = () => {
    const urls = [];
    for (const entry of performance.getEntriesByType('resource')) {
      urls.push(entry.name);
    }
    document
      .querySelectorAll('link[rel="stylesheet"][href], script[src]')
      .forEach((element) => {
        const value = element.href || element.src;
        if (value) urls.push(value);
      });
    return urls;
  };

  navigator.serviceWorker
    .register('/sw.js')
    .then(() => navigator.serviceWorker.ready)
    .then((registration) => {
      post(registration, collect());
      if ('PerformanceObserver' in window) {
        try {
          const observer = new PerformanceObserver((list) => {
            post(
              registration,
              list.getEntries().map((entry) => entry.name),
            );
          });
          observer.observe({ type: 'resource', buffered: true });
        } catch (error) {
          console.error('resource observer failed', error);
        }
      }
      setTimeout(() => post(registration, collect()), 1500);
      setTimeout(() => post(registration, collect()), 5000);
    })
    .catch((error) => console.error('service worker registration failed', error));
}
