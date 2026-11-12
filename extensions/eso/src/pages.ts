// ESO's and NOIRLab's video pages (one Djangoplicity archive design, two themes) read into what
// the source needs: the videos a list shows, and a video's text, credit, facts and files. No
// requests here.
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
  /** "Release date", "Duration"... as the page's table gives them. */
  facts: Record<string, string>;
  files: VideoFile[];
  subtitles: { url: string; lang: string }[];
  /** The page's picture (`og:image`), over https. */
  picture: string | null;
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

/** A video's id from its address on the site: `/public/videos/eso2613a/` is `eso2613a`. */
const videoId = (address: string | null) =>
  /^\/public\/videos\/([^/?#]+)\/$/.exec(address ?? '')?.[1] ?? null;

/**
 * The videos on a list, search or category page: thumbnails whose link names the video
 * (`data-url="/public/videos/eso2613a/"`) and its title (`data-title`).
 */
export const thumbnails = (html: string, base: string): Listed[] => {
  const seen = new Set<string>();
  return raygun.html.select(html, 'a[data-url]').flatMap((link) => {
    const id = videoId(link.attr('data-url'));
    const title = decode(link.attr('data-title') ?? '')
      .replace(/\s+/g, ' ')
      .trim();
    if (!id || !ID.test(id) || !title || seen.has(id)) return [];
    seen.add(id);
    const picture = link.select('img')[0]?.attr('src') ?? '';
    return [{ id, title, coverUrl: picture ? resolveUrl(base, picture) : null }];
  });
};

/** The videos on a "view all" page: a table of thumbnails, each titled by its picture's `alt`. */
export const tableRows = (html: string, base: string): Listed[] => {
  const seen = new Set<string>();
  return raygun.html.select(html, 'td.imagerow a[href]').flatMap((link) => {
    const id = videoId(link.attr('href'));
    const picture = link.select('img')[0];
    const title = decode(picture?.attr('alt') ?? '')
      .replace(/\s+/g, ' ')
      .trim();
    if (!id || !ID.test(id) || !title || seen.has(id)) return [];
    seen.add(id);
    const src = picture?.attr('src') ?? '';
    return [{ id, title, coverUrl: src ? resolveUrl(base, src) : null }];
  });
};

/** Whether a list page links to the page after `page` (`/list/3/` or `/page/3/`). */
export const hasPageAfter = (html: string, page: number) =>
  new RegExp(`/(?:list|page)/${page + 1}/`).test(html);

/** A video's page read, or null when it isn't one (no title). */
export const videoPage = (html: string, base: string, id: string): VideoPage | null => {
  const title = decode(raygun.html.select(html, 'h1')[0]?.text ?? '')
    .replace(/\s+/g, ' ')
    .trim();
  if (!title || /^not found$/i.test(title)) return null;
  // The text sits between the title (with the embedded player) and the credit.
  const start = html.indexOf('</h1>');
  const creditAt = html.indexOf('<div class="credit">');
  const between = creditAt > start ? html.slice(start, creditAt) : '';
  // Links to the news release and notes for the press aren't the video's text.
  const description = raygun.html
    .select(between, 'p')
    .map((paragraph) => decode(paragraph.text).replace(/\s+/g, ' ').trim())
    .filter(
      (text, index, all) =>
        text &&
        !/^(?:learn more about\b|members of the media\b|more information\b)/i.test(text) &&
        all.indexOf(text) === index,
    )
    .join('\n\n');
  const credit = lines(raygun.html.select(html, 'div.credit')[0]?.inner ?? '');
  const facts: Record<string, string> = {};
  for (const row of raygun.html.select(html, 'tr')) {
    const cells = /<t[hd][^>]*>([\s\S]*?)<\/t[hd]>\s*<td[^>]*>([\s\S]*?)<\/td>/i.exec(row.inner);
    if (!cells) continue;
    const name = lines(cells[1] ?? '')
      .join(' ')
      .replace(/:$/, '');
    if (name) facts[name] = lines(cells[2] ?? '').join(' ');
  }
  const files: VideoFile[] = raygun.html
    .select(html, 'span.archive_dl_text a[href]')
    .flatMap((link) => {
      const url = resolveUrl(base, link.attr('href') ?? '');
      const folder = /\/videos\/([a-z0-9_]+)\/[^/]+$/i.exec(url)?.[1];
      return folder ? [{ url, folder, name: decode(link.text).replace(/\s+/g, ' ').trim() }] : [];
    });
  const subtitles = files.flatMap((file) => {
    const name = file.url.split('/').pop() ?? '';
    if (file.folder !== 'srt' || !name.startsWith(id) || !/\.srt$/i.test(name)) return [];
    const lang = name.slice(id.length, -'.srt'.length).replace(/^[-_.]/, '');
    return lang ? [{ url: file.url, lang }] : [];
  });
  const og = /<meta\s+property="og:image(?::secure_url)?"\s+content="(https?:\/\/[^"]+)"/i.exec(
    html,
  )?.[1];
  const picture = og ? og.replace(/^http:\/\//i, 'https://') : null;
  return { title, description, credit, facts, files, subtitles, picture };
};
