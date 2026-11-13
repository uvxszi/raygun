import type { ExtensionsService } from '../extensions/service';
import { webRefusal } from '../extensions/webSources';
import { openCore } from '../test/webCore';
import { CatalogService, filtersOf, listingOf, orderLinks, stableId, titleIdOf } from './catalog';
import { blocksFromHtml, blocksFromText, passageBlocks } from './passage';

type Answers = Record<string, (args: Record<string, unknown>) => unknown>;

/** Sources answering from tables: `<provider id> <method>` → answer. */
const fakeSources = (answers: Answers, kind = 'manga.source') => {
  const calls: string[] = [];
  const extensions = {
    listed: async (asked: string) =>
      asked === kind
        ? [
            {
              info: {
                id: 'dev.test/books',
                extensionId: 'dev.test',
                kind,
                name: 'Test books',
                searchPill: null,
                adult: false,
              },
              enabled: true,
            },
          ]
        : [],
    call: async (providerId: string, method: string, args: Record<string, unknown>) => {
      calls.push(`${providerId} ${method}`);
      const answer = answers[method];
      if (!answer) throw new Error(`provider books has no method ${method}`);
      return answer(args);
    },
    nameOf: () => 'Test books',
  };
  return { extensions: extensions as unknown as ExtensionsService, calls };
};

const setUp = async (answers: Answers, kind?: string) => {
  const open = await openCore();
  const { extensions, calls } = fakeSources(answers, kind);
  const opened: Array<Record<string, unknown>> = [];
  const catalog = new CatalogService(
    open.core.context,
    extensions,
    () => open.core.settings.stored(),
    (request) => opened.push(request),
  );
  return { open, catalog, calls, opened };
};

const BOOK = {
  details: () => ({
    title: 'Frankenstein',
    authors: ['Mary Shelley'],
    status: 'completed',
    coverUrl: 'https://books.test/f.jpg',
  }),
  chapters: () => ({
    chapters: [
      { id: 'c2', name: 'Letter 2', number: 2 },
      { id: 'c1', number: 1 },
    ],
  }),
  popular: () => ({
    items: [
      { id: 'f', title: 'Frankenstein', coverUrl: 'https://books.test/f.jpg' },
      { id: 'x', title: 'Hidden', nsfw: true },
      { title: 'No id' },
    ],
    hasNextPage: true,
  }),
  pages: ({ chapterId }: Record<string, unknown>) => ({
    pages: [`https://books.test/${String(chapterId)}/1.jpg`, { url: 'https://books.test/2.jpg' }],
  }),
  passage: () => ({ html: '<h2>Letter 1</h2><p>To Mrs. <i>Saville</i>,</p><p>England.</p>' }),
};

describe('catalogue sources in the browser', () => {
  it('reads listings, chapters and filters as the app’s host does', () => {
    expect(listingOf({ items: [{ id: 3, title: ' A ' }, { id: 'b' }], hasNextPage: 1 })).toEqual({
      items: [expect.objectContaining({ id: '3', title: 'A', coverUrl: null })],
      hasNextPage: false,
    });
    expect(
      filtersOf([
        { kind: 'select', key: 'sort', options: ['new', { value: 'old', label: 'Oldest' }] },
        { kind: 'toggle', key: 'done', label: 'Finished', default: true },
        { kind: 'mystery', key: 'm' },
      ]),
    ).toEqual([
      {
        kind: 'select',
        key: 'sort',
        label: 'sort',
        options: [
          { value: 'new', label: 'new' },
          { value: 'old', label: 'Oldest' },
        ],
        default: null,
      },
      { kind: 'toggle', key: 'done', label: 'Finished', default: true },
    ]);
    expect(stableId('t', 'a')).toMatch(/^t[0-9a-f]{16}$/);
    expect(stableId('t', 'a')).not.toBe(stableId('t', 'b'));
  });

  it('lists a source’s titles without adult ones while they’re hidden', async () => {
    const { catalog } = await setUp(BOOK);
    const page = await catalog.browse('manga', 'dev.test/books', 'popular', null, 1);
    expect(page.hasNextPage).toBe(true);
    expect(page.items.map((item) => item.title)).toEqual(['Frankenstein']);
    expect(await catalog.sources('manga')).toEqual([
      expect.objectContaining({ id: 'dev.test/books', enabled: true }),
    ]);
  });

  it('falls back to the popular list for a source without a latest one', async () => {
    const { catalog, calls } = await setUp(BOOK);
    await catalog.browse('manga', 'dev.test/books', 'latest', null, 2);
    expect(calls).toEqual(['dev.test/books latest', 'dev.test/books popular']);
  });

  it('keeps a title opened, saves it to the library and reads its chapters', async () => {
    const { catalog, open } = await setUp(BOOK);
    const view = await catalog.mangaTitle('manga', 'dev.test/books', 'f', false);
    expect(view.id).toBe(titleIdOf('manga', 'dev.test/books', 'f'));
    expect(view.chapters.map((chapter) => chapter.name)).toEqual(['Letter 2', 'Chapter 1']);
    // Reading starts with the lowest number.
    expect(view.continueChapterId).toBe(view.chapters[1]?.id);
    expect(catalog.library('manga')).toEqual([]);

    catalog.setInLibrary(view.id, true);
    expect(catalog.library('manga')).toEqual([
      expect.objectContaining({ title: 'Frankenstein', chapters: 2, unread: 2 }),
    ]);
    const card = (await catalog.browse('manga', 'dev.test/books', 'popular', null, 1)).items[0];
    expect(card?.inLibrary).toBe(true);

    const first = view.chapters[1]!.id;
    const pages = await catalog.chapterPages(view.id, first);
    expect(pages.pages).toEqual(['https://books.test/c1/1.jpg', 'https://books.test/2.jpg']);
    expect(pages.nextChapterId).toBe(view.chapters[0]?.id);
    expect(pages.readerMode).toBe('webtoon');

    catalog.setProgress(first, 1, 2);
    expect(catalog.continueChapter(view.id)).toBe(view.chapters[0]?.id);
    expect(catalog.marks(view.id).inLibrary).toBe(true);

    // Kept across a reload.
    const saved = await open.saved();
    expect(saved.titles?.length).toBe(1);
  });

  it('reads a book chapter’s text as blocks', async () => {
    const { catalog } = await setUp(BOOK, 'novel.source');
    const view = await catalog.mangaTitle('novel', 'dev.test/books', 'f', false);
    const chapter = await catalog.novelChapter(view.id, view.chapters[0]!.id);
    expect(chapter.blocks).toEqual([
      { kind: 'heading', text: 'Letter 1' },
      { kind: 'paragraph', text: 'To Mrs. Saville,' },
      { kind: 'paragraph', text: 'England.' },
    ]);
  });

  it('plays an episode’s best link in the page’s player and keeps where it got to', async () => {
    const { catalog, opened } = await setUp(
      {
        details: () => ({ title: 'A film', type: 'movie' }),
        episodes: () => ({ episodes: [{ id: 'e1', name: 'A film' }] }),
        servers: () => ({ servers: [{ id: 's1', name: 'Archive' }] }),
        videos: () => ({
          videos: [
            { url: 'https://films.test/2160.mp4', quality: 2160 },
            { url: 'https://films.test/480.mp4', quality: 480 },
            { url: 'magnet:?xt=1', format: 'torrent' },
          ],
        }),
      },
      'video.source',
    );
    const view = await catalog.videoTitle('dev.test/books', 'film', false);
    await catalog.watch(view.id, 'e1');
    expect(opened).toEqual([
      expect.objectContaining({
        id: view.episodes[0]?.id,
        url: 'https://films.test/480.mp4',
        title: 'A film',
        subtitle: null,
      }),
    ]);
    catalog.report(
      JSON.stringify({
        id: view.episodes[0]?.id,
        state: 'playing',
        positionMs: 990,
        durationMs: 1000,
      }),
    );
    const after = await catalog.videoTitle('dev.test/books', 'film', false);
    expect(after.episodes[0]).toEqual(
      expect.objectContaining({ positionMs: 990, durationMs: 1000, watched: true }),
    );
  });

  it('orders links by the preferred resolution', () => {
    const link = (quality: number | null) => ({
      url: `https://v.test/${quality}`,
      label: null,
      quality,
      headers: {},
      format: null,
      subtitles: [],
      preferred: false,
    });
    expect(orderLinks([link(2160), link(720), link(1080)], 1080).map((l) => l.quality)).toEqual([
      1080, 720, 2160,
    ]);
  });
});

describe('book text in the browser', () => {
  it('keeps text, headings, code and pictures and drops scripts', () => {
    expect(
      blocksFromHtml(
        '<div>Intro<script>alert(1)</script><br>line<img src="/a.png"><img src="javascript:x"><pre>\nx = 1\n</pre><hr></div>',
        'https://book.test/ch/1.html',
      ),
    ).toEqual([
      { kind: 'paragraph', text: 'Intro' },
      { kind: 'paragraph', text: 'line' },
      { kind: 'image', url: 'https://book.test/a.png' },
      { kind: 'code', text: 'x = 1' },
      { kind: 'separator' },
    ]);
    expect(blocksFromText('One\n\nTwo')).toEqual([
      { kind: 'paragraph', text: 'One' },
      { kind: 'paragraph', text: 'Two' },
    ]);
    expect(() => passageBlocks({ pdf: 'https://book.test/b.pdf' })).toThrow(/PDF/);
  });
});

describe('which sources run in a browser', () => {
  it('marks sources whose sites refuse web pages, and kinds the web doesn’t use', () => {
    expect(webRefusal('dev.raygun.xkcd', ['manga.source'])).toMatch(/Raygun app/);
    expect(webRefusal('dev.raygun.fyyd', ['podcast.directory'])).toMatch(/kind of source/);
    expect(webRefusal('dev.raygun.internet-archive', ['music.metadata', 'video.source'])).toBe(
      null,
    );
  });
});
