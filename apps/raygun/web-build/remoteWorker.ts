// The service worker's part in Raygun on the web using a linked app (docs/WEB.md, "The app as a
// back end"): requests for the page's `__remote/…` paths, and for an app's proxy links still in
// their own form, go to the page (src/web/remote/media.ts), which fetches them through the app and
// answers on a MessagePort: the head, then a piece each time the browser reads on.
export const remoteWorker = `
const REMOTE = new URL('__remote/', self.registration.scope).href;
const APP_LINK = /^http:\\/\\/(?:127\\.0\\.0\\.1|localhost):\\d+\\/(?:stream|image|cover|subtitle|art|local|font|pdf|torrent|media)\\//;

const remotePath = (url) =>
  url.startsWith(REMOTE)
    ? url.slice(REMOTE.length)
    : APP_LINK.test(url)
      ? url.replace(/^http:\\/\\/[^/]+\\//, '')
      : null;

const pageFor = async (event) => {
  const own = event.clientId ? await self.clients.get(event.clientId) : null;
  if (own) return own;
  const pages = await self.clients.matchAll({ type: 'window' });
  return pages.find((page) => page.visibilityState === 'visible') || pages[0] || null;
};

const throughApp = async (event, path) => {
  const page = await pageFor(event);
  if (!page) return new Response('', { status: 503 });
  const { port1, port2 } = new MessageChannel();
  return new Promise((resolve) => {
    let body = null;
    let waiting = null;
    const wake = () => {
      const done = waiting;
      waiting = null;
      if (done) done();
    };
    port1.onmessage = ({ data }) => {
      if (data.head) {
        const empty = [204, 304].includes(data.head.status);
        resolve(
          new Response(
            empty
              ? null
              : new ReadableStream(
                  {
                    start: (controller) => {
                      body = controller;
                    },
                    pull: () =>
                      new Promise((done) => {
                        waiting = done;
                        port1.postMessage({ pull: true });
                      }),
                    cancel: () => {
                      port1.postMessage({ cancel: true });
                      port1.close();
                    },
                  },
                  { highWaterMark: 0 },
                ),
            { status: data.head.status, headers: data.head.headers },
          ),
        );
      } else if (data.chunk) {
        body.enqueue(new Uint8Array(data.chunk));
        wake();
      } else if (data.end) {
        body.close();
        wake();
        port1.close();
      } else if (data.error !== undefined) {
        if (body) body.error(new Error(data.error));
        else resolve(new Response(String(data.error), { status: 502 }));
        wake();
        port1.close();
      }
    };
    page.postMessage(
      { type: 'raygun-remote-fetch', path, range: event.request.headers.get('range') },
      [port2],
    );
  });
};

self.addEventListener('fetch', (event) => {
  if (event.request.method !== 'GET') return;
  const path = remotePath(event.request.url);
  if (path === null) return;
  event.respondWith(throughApp(event, path));
});
`;
