// __NAME__: site scripts (`site.scripts`) for Wikipedia's articles in Raygun's websites
// browser. They run in the page, with the page's own privileges and nothing of Raygun's: the
// manifest's `matches` say on which pages, `runAt` when (here once the article is parsed), and
// `styles/index.css` comes with them. `raygunPage` holds this extension's settings, the
// browser's state, and `offer`, which hands something on the page to Raygun as data.
//
// Try it: add en.wikipedia.org in Browse → Sources → Websites, then
// `pnpm exec raygun-ext test https://en.wikipedia.org/wiki/Ada_Lovelace` says what runs there,
// and `--bundle out.js` writes the script Raygun injects, to paste into a browser's console.
import type {} from '@raygun/sdk/site';

const { readingTime, wordsPerMinute } = raygunPage.settings;

/** The article's words, counted in its main text. */
const wordsIn = (element: Element) =>
  (element.textContent ?? '').split(/\s+/).filter((word) => word.length > 0).length;

const article = document.querySelector('#mw-content-text');
const heading = document.querySelector('#firstHeading');
if (readingTime !== false && article && heading) {
  const perMinute = typeof wordsPerMinute === 'number' && wordsPerMinute > 0 ? wordsPerMinute : 230;
  const minutes = Math.max(1, Math.round(wordsIn(article) / perMinute));
  const note = document.createElement('p');
  note.className = 'raygun-reading-time';
  note.textContent = `${minutes} min read`;
  heading.after(note);
}
