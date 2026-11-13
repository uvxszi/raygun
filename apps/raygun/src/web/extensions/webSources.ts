// Which sources run in a browser (docs/WEB.md). A browser fetches for the sources, so a source
// works only where its sites let web pages read them (CORS). These lists were measured by
// installing every Paraline source in the web build and asking each for a listing, a title and a
// chapter or video.

/** First-party sources a browser gets on its first start, as the apps get Paraline's. */
export const WEB_STARTER_SOURCES = [
  'dev.raygun.internet-archive',
  'dev.raygun.musicbrainz',
  'dev.raygun.listenbrainz',
  'dev.raygun.wikimedia-commons',
  'dev.raygun.live-music-archive',
  'dev.raygun.great-78',
  'dev.raygun.blender-movies',
  'dev.raygun.esa-hubble',
  'dev.raygun.eso',
  'dev.raygun.nasa',
  'dev.raygun.loc',
  'dev.raygun.peertube',
  'dev.raygun.green-tea-press',
  'dev.raygun.saylor',
  'dev.raygun.standard-ebooks',
  'dev.raygun.wikisource',
];

/** First-party sources whose sites don't let web pages read what they need. */
export const APP_ONLY_SOURCES = new Set([
  'dev.raygun.archive-books', // its chapters are PDF documents, drawn by the app
  'dev.raygun.archive-comics', // the Archive's file servers send pages without CORS
  'dev.raygun.bccampus',
  'dev.raygun.gnu-manuals',
  'dev.raygun.gutenberg',
  'dev.raygun.incompetech',
  'dev.raygun.nasa-svs',
  'dev.raygun.open-webcomics',
  'dev.raygun.openstax',
  'dev.raygun.peppercarrot',
  'dev.raygun.ucsb-cylinders',
  'dev.raygun.wikibooks',
  'dev.raygun.xkcd',
]);

/** The kinds of provider the web core calls. */
export const WEB_KINDS = new Set([
  'music.metadata',
  'music.stream',
  'music.charts',
  'manga.source',
  'novel.source',
  'video.source',
]);

export const NEEDS_APP_SITE =
  "Its site doesn't let web pages read it, so it works only in the Raygun app.";
export const NEEDS_APP_KIND = 'Raygun on the web doesn’t use this kind of source yet.';

/** Why an entry can't run in a browser, or null when it can. */
export const NEEDS_APP_PDF = 'Its books are PDF documents, which only the Raygun app shows.';

export const webRefusal = (id: string, provides: string[] | undefined): string | null => {
  if (id === 'dev.raygun.archive-books') return NEEDS_APP_PDF;
  if (APP_ONLY_SOURCES.has(id)) return NEEDS_APP_SITE;
  if (provides && provides.length > 0 && !provides.some((kind) => WEB_KINDS.has(kind))) {
    return NEEDS_APP_KIND;
  }
  return null;
};
