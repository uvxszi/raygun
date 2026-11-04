import type {
  MangaChapterInfo,
  MangaChapterPages,
  MangaReaderPrefs,
  ReaderMode,
} from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../../test/bindingsMock';

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
    expect(
      within(within(reader).getByTestId('reader-bar')).getByRole('button', {
        name: 'Back to the fit',
      }),
    ).toHaveTextContent('200%');
  });

  it('shows a chapter without pages as such', async () => {
    commands.mangaChapter.mockResolvedValue(ok(chapter('pagedLtr', 0)));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    expect(await screen.findByText('This chapter has no pages.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});
