// ESA/Hubble's and ESA/Webb's video pages (one Djangoplicity archive design, two themes) read into
// what the source needs: the videos a list shows, and a video's text, credit, facts and files.
// No requests here.
import { resolveUrl } from '@raygun/sdk';

/** A video as a list shows it. */
export type Listed = { id: string; title: string; coverUrl: string | null };

/** A video file the site offers, by the folder it's in (`hd_1080p25_screen`, `medium_podcast`...). */
export type VideoFile = { url: string; folder: string; name: string };

/** What a video's own page says of it. */
export type VideoPage = {
  title: string;
  /** Its paragraphs, as text. */
  description: string;
  /** The credit's lines: who made it, then "Music: ..." when it has music. */
  credit: string[];
  /** "Release date", "Duration", "Name", "Type"... as the page's tables give them. */
  facts: Record<string, string>;
  categories: string[];
  files: VideoFile[];
  subtitles: { url: string; lang: string }[];
};

const ID = /^[A-Za-z0-9_-]+$/;

/** Text with the entities an element's inner HTML keeps (`&amp;`, `&nbsp;`...) written out. */
const decode = (text: string) =>
  text
    .replace(/&#x([0-9a-f]+);/gi, (_, hex: string) =>
      String.fromCodePoint(Number.parseInt(hex, 16)),
    )
    .replace(/&#(\d+);/g, (_, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');

/** HTML as lines of text: paragraphs and line breaks kept, tags dropped. */
const lines = (html: string): string[] =>
  decode(
    html
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/(?:p|div|li|h\d)>/gi, '\n')
      .replace(/<[^>]*>/g, ''),
  )
    .split('\n')
    .map((line) => line.replace(/\s+/g, ' ').trim())
    .filter(Boolean);

/**
 * The videos on a list, search or category page: thumbnails whose link names the video
 * (`data-url="/videos/heic2021a/"`) and its title (`data-title`).
 */
export const thumbnails = (html: string): Listed[] => {
  const seen = new Set<string>();
  return raygun.html.select(html, 'a[data-url]').flatMap((link) => {
    const id = /^\/videos\/([^/?#]+)\/$/.exec(link.attr('data-url') ?? '')?.[1];
    const title = (link.attr('data-title') ?? '').replace(/\s+/g, ' ').trim();
    if (!id || !ID.test(id) || !title || seen.has(id)) return [];
    seen.add(id);
    const picture = link.select('img')[0]?.attr('src') ?? '';
    return [{ id, title, coverUrl: /^https?:\/\//i.test(picture) ? picture : null }];
  });
};

/** The videos on a "view all" page: a table of thumbnails, each titled by its picture's `alt`. */
export const tableRows = (html: string): Listed[] => {
  const seen = new Set<string>();
  return raygun.html.select(html, 'td.imagerow a[href]').flatMap((link) => {
    const id = /^\/videos\/([^/?#]+)\/$/.exec(link.attr('href') ?? '')?.[1];
    const title = (link.select('img')[0]?.attr('alt') ?? '').replace(/\s+/g, ' ').trim();
    if (!id || !ID.test(id) || !title || seen.has(id)) return [];
    seen.add(id);
    return [{ id, title, coverUrl: null }];
  });
};

/** Whether a list page links to the page after `page`. */
export const hasPageAfter = (html: string, page: number) => html.includes(`/page/${page + 1}/`);

/** A video's page read, or null when it isn't one (no title). */
export const videoPage = (html: string, base: string, id: string): VideoPage | null => {
  const title = raygun.html.select(html, 'h1')[0]?.text.replace(/\s+/g, ' ').trim();
  if (!title) return null;
  // The text sits between the title (with the embedded player) and the credit.
  const start = html.indexOf('</h1>');
  const creditAt = html.indexOf('<div class="credit">');
  const between = creditAt > start ? html.slice(start, creditAt) : '';
  // "Learn more about the image here." links to the news release, which details don't show.
  const description = raygun.html
    .select(between, 'p')
    .map((paragraph) => paragraph.text.replace(/\s+/g, ' ').trim())
    .filter(
      (text, index, all) =>
        text && !/^learn more about\b/i.test(text) && all.indexOf(text) === index,
    )
    .join('\n\n');
  const credit = lines(raygun.html.select(html, 'div.credit')[0]?.inner ?? '');
  const facts: Record<string, string> = {};
  const categories: string[] = [];
  for (const row of raygun.html.select(html, 'tr')) {
    const cells = /<th[^>]*>([\s\S]*?)<\/th>\s*<td[^>]*>([\s\S]*?)<\/td>/i.exec(row.inner);
    if (!cells) continue;
    const name = lines(cells[1] ?? '')
      .join(' ')
      .replace(/:$/, '');
    if (name === 'Category') {
      for (const link of (cells[2] ?? '').matchAll(/<a[^>]*>([\s\S]*?)<\/a>/gi)) {
        const category = lines(link[1] ?? '').join(' ');
        if (category && !categories.includes(category)) categories.push(category);
      }
    } else if (name) {
      facts[name] = lines(cells[2] ?? '').join(' ');
    }
  }
  const files: VideoFile[] = raygun.html
    .select(html, 'span.archive_dl_text a[href]')
    .flatMap((link) => {
      const url = resolveUrl(base, link.attr('href') ?? '');
      const folder = /\/archives\/videos\/([a-z0-9_]+)\/[^/]+$/i.exec(url)?.[1];
      return folder ? [{ url, folder, name: link.text.replace(/\s+/g, ' ').trim() }] : [];
    });
  const subtitles = files.flatMap((file) => {
    const name = file.url.split('/').pop() ?? '';
    if (file.folder !== 'srt' || !name.startsWith(id) || !/\.srt$/i.test(name)) return [];
    const lang = name.slice(id.length, -'.srt'.length).replace(/^[-_.]/, '');
    return lang ? [{ url: file.url, lang }] : [];
  });
  return { title, description, credit, facts, categories, files, subtitles };
};
