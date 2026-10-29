// SAYL Download Manager - Alt-click bypass content script
// Portions derived from AB Download Manager (https://github.com/amir1376/ab-download-manager), Apache-2.0
(() => {
  const TTL_MS = 10000;

  window.addEventListener(
    "click",
    (event) => {
      if (!event.altKey) return;
      const target = event.target;
      const anchor = target && target.closest ? target.closest("a[href]") : null;
      if (!anchor || !anchor.href) return;

      const url = anchor.href;
      if (!/^https?:\/\//i.test(url)) return;

      const now = Date.now();
      try {
        if (!chrome.storage || !chrome.storage.session) return;
        chrome.storage.session.get("alt_clicks", (items) => {
          if (chrome.runtime?.lastError) return;
          const records = (items && items.alt_clicks) || {};
          // Purge expired records
          for (const key of Object.keys(records)) {
            if (typeof records[key] === "number" && now - records[key] > TTL_MS) {
              delete records[key];
            }
          }
          records[url] = now;
          chrome.storage.session.set({ alt_clicks: records }, () => {
            const _ = chrome.runtime?.lastError;
          });
        });
      } catch (_) {}
    },
    true
  );
})();
