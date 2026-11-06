import type {
  MangaChapterInfo,
  MangaChapterPages,
  MangaReaderPrefs,
  ReaderMode,
  SettingChange,
  TapCell,
} from '@raygun/bindings';
import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, DEFAULT_SETTINGS } from '../../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const info = (id: string): MangaChapterInfo => ({
  id,
  name: `Chapter ${id}`,
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
});

const chapter = (mode: ReaderMode, count: number): MangaChapterPages => ({
  titleId: 't1',
  title: 'Frieren',
  readerMode: mode,
  ownReaderMode: mode,
  chapter: info('c1'),
  pages: Array.from({ length: count }, (_, index) => `http://img/${index + 1}`),
  previousChapterId: null,
  nextChapterId: null,
});

/** Reader settings on top of the defaults. */
const withReader = (prefs: MangaReaderPrefs) =>
  commands.settingsGet.mockResolvedValue(
    ok({ ...DEFAULT_SETTINGS, mangaReader: { ...DEFAULT_SETTINGS.mangaReader, ...prefs } }),
  );

type Entries = { isIntersecting: boolean; target: Element }[];

describe('The manga reader', () => {
  const observer: { notify?: (entries: Entries) => void } = {};

  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSetProgress.mockResolvedValue(ok(null));
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        constructor(callback: (entries: Entries) => void) {
          observer.notify = callback;
        }
        observe() {}
        disconnect() {}
      },
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('keeps only the pages near the one being read in the strip', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('webtoon', 20)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const slots = await within(reader).findAllByTestId('webtoon-page');
    expect(slots).toHaveLength(20);
    // The page, two behind it, and three to load ahead and one more.
    expect(within(reader).getAllByTestId('manga-page')).toHaveLength(5);

    observer.notify?.([{ isIntersecting: true, target: slots[10]! }]);
    await vi.waitFor(() =>
      expect(within(reader).getByTestId('page-indicator')).toHaveTextContent('Page 11 of 20'),
    );
    const shown = within(reader)
      .getAllByTestId('manga-page')
      .map((page) => page.getAttribute('data-index'));
    expect(shown).toEqual(['8', '9', '10', '11', '12', '13', '14']);
  });

  it('counts a short first page as the first, at the top of the strip and after scrolling', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('webtoon', 4)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const slots = await within(reader).findAllByTestId('webtoon-page');
    // A wide strip: pages 150 px tall on an 800 px screen, so the reading line (a third down)
    // crosses the second page while the first is at the top.
    let scrolled = 0;
    slots.forEach((slot, index) =>
      vi.spyOn(slot, 'getBoundingClientRect').mockImplementation(() => {
        const top = index * 150 - scrolled;
        return { top, bottom: top + 150, height: 150, left: 0, right: 400, width: 400 } as DOMRect;
      }),
    );
    const indicator = within(reader).getByTestId('page-indicator');
    act(() => observer.notify?.([{ isIntersecting: true, target: slots[1]! }]));
    expect(indicator).toHaveTextContent('Page 1 of 4');

    // Read on: the first page leaves the top, and the line crosses the third page while the
    // second's top is still on screen.
    scrolled = 100;
    observer.notify?.([
      { isIntersecting: false, target: slots[1]! },
      { isIntersecting: true, target: slots[2]! },
    ]);
    await vi.waitFor(() => expect(indicator).toHaveTextContent('Page 2 of 4'));
    // The second's top leaves too, without the line crossing anything new.
    scrolled = 200;
    fireEvent.scroll(screen.getByTestId('page-content'));
    await vi.waitFor(() => expect(indicator).toHaveTextContent('Page 3 of 4'));
  });

  it('counts the page shown in the paged modes, from the first', async () => {
    const user = userEvent.setup();
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const indicator = await within(reader).findByTestId('page-indicator');
    expect(indicator).toHaveTextContent('Page 1 of 3');
    expect(within(reader).getByTestId('manga-page')).toHaveAttribute('data-index', '0');
    await user.keyboard('{ArrowRight}');
    expect(indicator).toHaveTextContent('Page 2 of 3');
    expect(within(reader).getByTestId('manga-page')).toHaveAttribute('data-index', '1');
  });

  it('lists the chapters in the reader, sorted as asked, and reads the one picked there', async () => {
    const user = userEvent.setup();
    const numbered = (id: string, number: number, read = false): MangaChapterInfo => ({
      ...info(id),
      number,
      read,
    });
    // As the source lists them: newest first.
    commands.mangaChapters.mockResolvedValue(
      ok([numbered('c3', 3), numbered('c2', 2), numbered('c1', 1, true)]),
    );
    commands.mangaChapter.mockImplementation(async (_title: string, id: string) =>
      ok({ ...chapter('pagedLtr', 3), chapter: info(id) }),
    );
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await user.click(await within(reader).findByRole('button', { name: 'Chapters' }));
    const list = await screen.findByTestId('reader-chapters');
    expect(commands.mangaChapters).toHaveBeenCalledWith('t1');
    const names = () =>
      within(list)
        .getAllByTestId('reader-chapter')
        .map((row) => row.textContent);
    await vi.waitFor(() => expect(names()).toEqual(['Chapter c3', 'Chapter c2', 'Chapter c1']));
    expect(within(list).getByRole('button', { name: /Chapter c1/ })).toHaveAttribute(
      'aria-current',
      'true',
    );

    // Sorted the other way round, as on the title page.
    await user.click(within(list).getByRole('button', { name: 'Sort and filter chapters' }));
    await user.click(await screen.findByRole('button', { name: /Source order/ }));
    await vi.waitFor(() => expect(names()).toEqual(['Chapter c1', 'Chapter c2', 'Chapter c3']));

    await user.click(within(list).getByRole('button', { name: /Chapter c2/ }));
    await vi.waitFor(() => expect(commands.mangaChapter).toHaveBeenLastCalledWith('t1', 'c2'));
    expect(screen.queryByTestId('reader-chapters')).not.toBeInTheDocument();
    expect(await within(reader).findByText('Chapter c2')).toBeInTheDocument();
  });

  it('turns tapped zones of the strip into scrolls, and the middle into the controls', async () => {
    const user = userEvent.setup();
    withReader({ doubleTapZoom: false });
    commands.mangaChapter.mockResolvedValue(ok(chapter('webtoon', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findAllByTestId('webtoon-page');
    const main = screen.getByTestId('page-content');
    Object.defineProperty(main, 'clientHeight', { configurable: true, value: 800 });
    main.scrollBy = vi.fn();
    vi.spyOn(main, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      right: 400,
      bottom: 800,
      width: 400,
      height: 800,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    const strip = within(reader).getByTestId('webtoon-strip');
    const tapAt = (clientX: number, clientY: number) =>
      user.pointer({ keys: '[MouseLeft]', target: strip, coords: { clientX, clientY } });

    await tapAt(200, 750);
    expect(main.scrollBy).toHaveBeenLastCalledWith({ top: 680 });
    await tapAt(200, 40);
    expect(main.scrollBy).toHaveBeenLastCalledWith({ top: -680 });

    // The middle hides the controls; the page number stays.
    await tapAt(200, 400);
    expect(within(reader).queryByTestId('reader-bar')).not.toBeInTheDocument();
    expect(within(reader).getByTestId('page-number')).toHaveTextContent('1 / 3');
  });

  it('says when a page fails and loads it again', async () => {
    const user = userEvent.setup();
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    fireEvent.error(await within(reader).findByTestId('manga-page'));

    const failed = await within(reader).findByRole('alert');
    expect(failed).toHaveTextContent("Page 1 didn't load");
    await user.click(within(failed).getByRole('button', { name: 'Try again' }));
    expect(within(reader).getByTestId('manga-page')).toHaveAttribute('src', 'http://img/1?retry=1');
    expect(within(reader).queryByRole('alert')).not.toBeInTheDocument();
  });

  it('turns pages with keys, but not while the page slider has them', async () => {
    const user = userEvent.setup();
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 5)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const indicator = await within(reader).findByTestId('page-indicator');

    await user.keyboard(' ');
    expect(indicator).toHaveTextContent('Page 2 of 5');
    await user.keyboard('{End}');
    expect(indicator).toHaveTextContent('Page 5 of 5');
    await user.keyboard('{PageUp}');
    expect(indicator).toHaveTextContent('Page 4 of 5');

    within(reader).getByRole('slider', { name: 'Jump to page' }).focus();
    await user.keyboard('{ArrowRight}');
    expect(indicator).toHaveTextContent('Page 4 of 5');
  });

  it('zooms with two fingers, drawn while they move and laid out when they lift', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const page = await within(reader).findByTestId('manga-page');
    const finger = (x: number) => ({ clientX: x, clientY: 300 });

    fireEvent.touchStart(reader, { touches: [finger(100), finger(200)] });
    fireEvent.touchMove(reader, { touches: [finger(50), finger(250)] });
    expect(page.style.transform).toBe('scale(2)');
    fireEvent.touchEnd(reader, { touches: [finger(50)] });
    expect(page.style.transform).toBe('');
    expect(within(reader).getByTestId('manga-page').style.width).toBe('200%');
  });

  it('shows a chapter without pages as such', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 0)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    expect(await screen.findByText('This chapter has no pages.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('keeps one reading mode button and no zoom buttons on phones', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const bar = await within(reader).findByTestId('reader-bar');
    expect(within(reader).getAllByRole('button', { name: 'Reading mode' })).toHaveLength(1);
    expect(within(bar).queryByRole('button', { name: 'Reading mode' })).not.toBeInTheDocument();
    expect(within(bar).queryByRole('button', { name: 'Zoom in' })).not.toBeInTheDocument();
    expect(within(bar).queryByRole('button', { name: /full screen/i })).not.toBeInTheDocument();
    expect(within(bar).getByTestId('reader-progress')).toBeInTheDocument();
  });

  it('hides the progress when it is turned off', async () => {
    withReader({ barProgress: false });
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findByTestId('reader-bar');
    await vi.waitFor(() =>
      expect(
        within(reader).queryByRole('slider', { name: 'Jump to page' }),
      ).not.toBeInTheDocument(),
    );
    // The page count stays.
    expect(within(reader).getByTestId('page-number')).toHaveTextContent('1 / 3');
  });

  it('keeps the zoom and mode buttons on desktop', async () => {
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const bar = await within(reader).findByTestId('reader-bar');
    expect(await within(bar).findByRole('button', { name: 'Zoom in' })).toBeInTheDocument();
    expect(within(bar).getByRole('button', { name: 'Reading mode' })).toBeInTheDocument();
  });

  it('fits the strip and the page to the container width exactly', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('webtoon', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const strip = await within(reader).findByTestId('webtoon-strip');
    // A share of what holds it (never the viewport's 100vw), with nothing added beside it.
    expect(strip.style.width).toBe('min(100%, 48rem)');
    expect(strip.className).not.toMatch(/\b(p|px|pl|pr|m|ml|mr)-\d/);
    expect(strip.style.transform).toBe('');
    for (const page of within(reader).getAllByTestId('manga-page')) {
      expect(page).toHaveClass('w-full');
    }
    // The reader cancels the page area's padding and draws no scrollbar there (styles.css).
    expect(reader).toHaveAttribute('data-edge-reader');
    expect(reader).toHaveClass('-mx-4');
    expect(screen.getByTestId('page-content')).toHaveClass('px-4');
  });

  it('fits a paged page to the width exactly', async () => {
    withReader({ pagedScale: 'fitWidth' });
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const paged = await within(reader).findByTestId('paged-reader');
    await vi.waitFor(() => expect(paged).toHaveAttribute('data-scale', 'fitWidth'));
    expect(within(paged).getByTestId('manga-page').style.width).toBe('100%');
    expect(paged).toHaveClass('w-full');
  });

  describe('double taps', () => {
    const mountStrip = async () => {
      commands.mangaChapter.mockResolvedValue(ok(chapter('webtoon', 3)));
      await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
      const reader = await screen.findByTestId('manga-reader');
      const strip = await within(reader).findByTestId('webtoon-strip');
      const main = screen.getByTestId('page-content');
      const scrolled = { left: 0, top: 0 };
      Object.defineProperty(main, 'scrollLeft', {
        configurable: true,
        get: () => scrolled.left,
        set: (value: number) => (scrolled.left = value),
      });
      Object.defineProperty(main, 'scrollTop', {
        configurable: true,
        get: () => scrolled.top,
        set: (value: number) => (scrolled.top = value),
      });
      main.scrollBy = vi.fn();
      vi.spyOn(strip, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 400, 2000));
      const doubleTap = (clientX: number, clientY: number) => {
        fireEvent.click(strip, { clientX, clientY });
        fireEvent.click(strip, { clientX, clientY });
      };
      return { strip, main, scrolled, doubleTap };
    };

    it('zooms in about twice, anchored at the point tapped, then back to the fit', async () => {
      const { strip, main, scrolled, doubleTap } = await mountStrip();
      doubleTap(100, 200);
      expect(strip.style.width.replace(/ /g, '')).toBe('calc(min(100%,48rem)*2)');
      // The tapped point stays under the finger: it moved from (100, 200) to (200, 400).
      expect(scrolled).toEqual({ left: 100, top: 200 });

      doubleTap(150, 300);
      expect(strip.style.width).toBe('min(100%, 48rem)');
      // Neither tap of a double tap scrolls or turns anything.
      await new Promise((resolve) => setTimeout(resolve, 300));
      expect(main.scrollBy).not.toHaveBeenCalled();
    });

    it('goes back to the fit when zoomed by other means', async () => {
      const user = userEvent.setup();
      const { strip, doubleTap } = await mountStrip();
      await user.keyboard('{Control>}={/Control}');
      expect(strip.style.width.replace(/ /g, '')).toBe('calc(min(100%,48rem)*1.1)');
      doubleTap(100, 200);
      expect(strip.style.width).toBe('min(100%, 48rem)');
    });

    it('treats taps far apart as two taps', async () => {
      const { strip } = await mountStrip();
      fireEvent.click(strip, { clientX: 20, clientY: 20 });
      fireEvent.click(strip, { clientX: 380, clientY: 700 });
      expect(strip.style.width).toBe('min(100%, 48rem)');
    });
  });

  it('shows only the page actions on a long press or right click', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const page = await within(reader).findByTestId('manga-page');
    expect(fireEvent.contextMenu(page)).toBe(false);
    expect(await screen.findByRole('dialog', { name: 'Page 1' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: /Back|Reload/ })).not.toBeInTheDocument();
    // Around the page too: nothing of the shell's.
    expect(fireEvent.contextMenu(within(reader).getByTestId('paged-reader'))).toBe(false);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(reader).toHaveClass('select-none');
    expect(page).toHaveAttribute('draggable', 'false');
  });

  it('hides the system bars while reading and shows them with the menu', async () => {
    const user = userEvent.setup();
    withReader({ doubleTapZoom: false });
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const paged = await within(reader).findByTestId('paged-reader');
    expect(commands.readerSystemBars).not.toHaveBeenCalled();
    await user.click(paged);
    await vi.waitFor(() => expect(commands.readerSystemBars).toHaveBeenLastCalledWith(true));
    await user.click(paged);
    await vi.waitFor(() => expect(commands.readerSystemBars).toHaveBeenLastCalledWith(false));
  });

  it('keeps sideways finger moves for its swipes', async () => {
    withReader({ doubleTapZoom: false });
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    // A browser pan would cancel the finger's pointer before the swipe lifts.
    expect(await within(reader).findByTestId('paged-reader')).toHaveStyle({
      touchAction: 'pan-y',
    });
  });

  describe('custom tap zones', () => {
    const grid = (cells: Record<number, TapCell>) =>
      Array.from({ length: 9 }, (_, index) => cells[index] ?? 'menu') as unknown as NonNullable<
        MangaReaderPrefs['tapGridPaged']
      >;
    const tapAt = (target: HTMLElement, clientX: number, clientY: number) =>
      fireEvent.click(target, { clientX, clientY });
    const rect = (element: HTMLElement) =>
      vi.spyOn(element, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 300, 900));
    const saved = () =>
      commands.settingsUpdate.mock.calls
        .map(([change]) => change as SettingChange)
        .filter((change) => change.key === 'mangaReader')
        .map((change) => change.value as MangaReaderPrefs)
        .at(-1);

    it('turns pages and jumps in the paged modes', async () => {
      withReader({
        doubleTapZoom: false,
        tapZones: 'custom',
        tapGridPaged: grid({
          0: 'firstPage',
          2: 'lastPage',
          3: 'previous',
          5: 'next',
          8: 'nothing',
        }),
      });
      commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 5)));
      await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
      const reader = await screen.findByTestId('manga-reader');
      const indicator = await within(reader).findByTestId('page-indicator');
      const paged = within(reader).getByTestId('paged-reader');
      rect(paged);

      tapAt(paged, 250, 450);
      expect(indicator).toHaveTextContent('Page 2 of 5');
      tapAt(paged, 250, 50);
      expect(indicator).toHaveTextContent('Page 5 of 5');
      tapAt(paged, 50, 450);
      expect(indicator).toHaveTextContent('Page 4 of 5');
      tapAt(paged, 50, 50);
      expect(indicator).toHaveTextContent('Page 1 of 5');
      // Nothing does nothing: the bars stay.
      tapAt(paged, 250, 850);
      expect(within(reader).getByTestId('reader-bar')).toBeInTheDocument();
    });

    it('scrolls the strip a screen and moves between chapters', async () => {
      withReader({
        doubleTapZoom: false,
        tapZones: 'custom',
        invertTaps: 'vertical',
        tapGridWebtoon: grid({ 0: 'next', 6: 'previous', 2: 'nextChapter' }),
      });
      commands.mangaChapter.mockResolvedValue(
        ok({ ...chapter('webtoon', 3), nextChapterId: 'c2' }),
      );
      await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
      const reader = await screen.findByTestId('manga-reader');
      const strip = await within(reader).findByTestId('webtoon-strip');
      const main = screen.getByTestId('page-content');
      Object.defineProperty(main, 'clientHeight', { configurable: true, value: 900 });
      main.scrollBy = vi.fn();
      rect(main);

      // Mirrored top to bottom: the bottom left is the top left's Next, a screen less an overlap.
      tapAt(strip, 50, 850);
      expect(main.scrollBy).toHaveBeenLastCalledWith({ top: 765 });
      tapAt(strip, 50, 50);
      expect(main.scrollBy).toHaveBeenLastCalledWith({ top: -765 });
      commands.mangaChapter.mockResolvedValue(
        ok({ ...chapter('webtoon', 3), chapter: info('c2') }),
      );
      tapAt(strip, 250, 850);
      await vi.waitFor(() => expect(commands.mangaChapter).toHaveBeenLastCalledWith('t1', 'c2'));
    });

    it('edits each grid in the Display sheet', async () => {
      const user = userEvent.setup();
      withReader({ tapZones: 'custom' });
      commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
      await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
      const reader = await screen.findByTestId('manga-reader');
      await user.click(await within(reader).findByRole('button', { name: 'Display' }));
      const sheet = await screen.findByRole('dialog', { name: 'Display' });
      const editor = await within(sheet).findByTestId('tap-grid-editor');
      const preview = within(editor).getByRole('group', { name: 'Tap zones for Pages' });
      expect(within(preview).getAllByRole('button')).toHaveLength(9);

      await user.click(within(preview).getByRole('button', { name: 'Row 1, column 3: Right' }));
      await user.click(within(editor).getByRole('radio', { name: 'Last page' }));
      expect(saved()?.tapGridPaged?.[2]).toBe('lastPage');
      // The strip's grid is kept as it was.
      expect(saved()?.tapGridWebtoon?.[0]).toBe('previous');

      // Presets and Reset start the grid over; the strip has its own.
      await user.click(within(editor).getByRole('button', { name: 'Kindle' }));
      expect(saved()?.tapGridPaged).toEqual([
        'menu',
        'menu',
        'menu',
        'previous',
        'next',
        'next',
        'previous',
        'next',
        'next',
      ]);
      await user.click(within(editor).getByRole('button', { name: 'Webtoon' }));
      expect(within(editor).getByRole('group', { name: 'Tap zones for Webtoon' })).toBeVisible();
      await user.click(within(editor).getByRole('button', { name: 'Off' }));
      expect(saved()?.tapGridWebtoon).toEqual(Array(9).fill('menu'));
      await user.click(within(editor).getByRole('button', { name: 'Reset' }));
      expect(saved()?.tapGridWebtoon).toEqual([
        'previous',
        'previous',
        'previous',
        'previous',
        'menu',
        'next',
        'next',
        'next',
        'next',
      ]);
      expect(saved()?.tapZones).toBe('custom');
    });

    it('opens every reader setting from the Display sheet', async () => {
      const user = userEvent.setup();
      commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 3)));
      await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
      const reader = await screen.findByTestId('manga-reader');
      await user.click(await within(reader).findByRole('button', { name: 'Display' }));
      const sheet = await screen.findByRole('dialog', { name: 'Display' });
      await user.click(within(sheet).getByRole('link', { name: 'All reader settings' }));
      expect(await screen.findByTestId('settings-reader-page')).toBeInTheDocument();
    });
  });
});
