import type { MangaLibraryEntry, MangaTitleView } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';
import { LIBRARY_DISPLAY_DEFAULTS } from './libraryDisplay';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.example.comics/comics';

const entry = (overrides: Partial<MangaLibraryEntry>): MangaLibraryEntry =>
  libraryEntry({ sourceId: SOURCE, ...overrides });

const LIBRARY = [
  entry({
    titleId: 'a',
    remoteId: 'a',
    title: 'Frieren',
    unread: 3,
    list: 'reading',
    categoryIds: ['c1'],
  }),
  entry({ titleId: 'b', remoteId: 'b', title: 'Berserk', favorite: true, list: 'completed' }),
  entry({ titleId: 'c', remoteId: 'c', title: 'Chainsaw Man', unread: 9, categoryIds: ['c1'] }),
];
const CATEGORIES = [
  { id: 'c1', name: 'Reading', display: null },
  { id: 'c2', name: 'Later', display: null },
];

const items = (container: HTMLElement) =>
  within(container)
    .queryAllByTestId('library-item')
    .map((item) => item.getAttribute('aria-label'));

const press = (user: ReturnType<typeof userEvent.setup>, target: HTMLElement) =>
  user.pointer({ keys: '[MouseRight]', target });

describe('Library depth', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaLibrary.mockResolvedValue(ok(LIBRARY));
    commands.libraryCategories.mockResolvedValue(ok(CATEGORIES));
  });

  it('selects with long presses and acts on the selection', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await within(library).findByText('Frieren');
    expect(items(library)).toEqual(['Berserk', 'Chainsaw Man', 'Frieren']);

    // A long press selects; another adds everything in between (Mihon's range).
    await press(user, within(library).getByRole('link', { name: 'Berserk' }));
    expect(within(library).getByRole('heading', { name: '1 selected' })).toBeInTheDocument();
    await press(user, within(library).getByRole('link', { name: 'Frieren' }));
    expect(within(library).getByRole('heading', { name: '3 selected' })).toBeInTheDocument();
    // While selecting, a tap toggles instead of opening.
    await user.click(within(library).getByRole('link', { name: 'Chainsaw Man' }));
    expect(within(library).getByRole('heading', { name: '2 selected' })).toBeInTheDocument();
    await user.click(within(library).getByRole('button', { name: 'Invert selection' }));
    expect(within(library).getByRole('heading', { name: '1 selected' })).toBeInTheDocument();
    await user.click(within(library).getByRole('button', { name: 'Select all' }));
    expect(within(library).getByRole('heading', { name: '3 selected' })).toBeInTheDocument();

    const actions = within(library).getByRole('toolbar', { name: 'Selected titles' });
    await user.click(within(actions).getByRole('button', { name: 'List' }));
    const lists = await screen.findByRole('dialog', { name: 'List' });
    await user.click(within(lists).getByRole('radio', { name: 'On hold' }));
    expect(commands.librarySetList).toHaveBeenCalledWith('manga', ['a', 'b', 'c'], 'onHold');
    // Acting ends selection.
    await vi.waitFor(() =>
      expect(within(library).queryByTestId('selection-bar')).not.toBeInTheDocument(),
    );

    await press(user, within(library).getByRole('link', { name: 'Frieren' }));
    await user.click(
      within(within(library).getByRole('toolbar')).getByRole('button', { name: 'Mark as read' }),
    );
    expect(commands.libraryMarkRead).toHaveBeenCalledWith('manga', ['a'], true);

    await press(user, within(library).getByRole('link', { name: 'Frieren' }));
    await press(user, within(library).getByRole('link', { name: 'Berserk' }));
    await user.click(
      within(within(library).getByRole('toolbar')).getByRole('button', { name: 'Set groups' }),
    );
    const sheet = await screen.findByRole('dialog', { name: 'Set groups' });
    // Only some of them are in Reading: its box starts mixed.
    const reading = within(sheet).getByRole('checkbox', { name: 'Reading' });
    expect(reading).toHaveAttribute('aria-checked', 'mixed');
    await user.click(reading);
    await user.click(reading);
    expect(reading).toHaveAttribute('aria-checked', 'true');
    await user.click(within(sheet).getByRole('checkbox', { name: 'Later' }));
    await user.click(within(sheet).getByRole('button', { name: 'OK' }));
    expect(commands.libraryChangeCategories).toHaveBeenCalledWith(
      'manga',
      expect.arrayContaining(['a', 'b', 'c']),
      ['c1', 'c2'],
      [],
    );

    await press(user, within(library).getByRole('link', { name: 'Berserk' }));
    await user.click(
      within(within(library).getByRole('toolbar')).getByRole('button', { name: 'Remove' }),
    );
    const remove = await screen.findByRole('dialog', { name: 'Remove' });
    const confirm = within(remove).getByRole('button', { name: 'Remove' });
    expect(confirm).toBeDisabled();
    await user.click(within(remove).getByRole('checkbox', { name: 'Downloaded chapters' }));
    await user.click(confirm);
    expect(commands.libraryRemove).toHaveBeenCalledWith('manga', ['b'], false, true);
  });

  it('downloads the next chapters of a selection', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await press(user, await within(library).findByRole('link', { name: 'Frieren' }));
    await user.click(
      within(within(library).getByRole('toolbar')).getByRole('button', { name: 'Download' }),
    );
    const sheet = await screen.findByRole('dialog', { name: 'Download' });
    await user.click(within(sheet).getByRole('button', { name: 'Next 5 chapters' }));
    expect(commands.libraryDownloadNext).toHaveBeenCalledWith('manga', ['a'], 5);
  });

  it('groups by list, lays titles out as a list and continues from a cover', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        libraryDisplay: { ...LIBRARY_DISPLAY_DEFAULTS, tabs: 'lists', mode: 'list' },
      }),
    );
    commands.libraryContinue.mockResolvedValue(ok('u7'));
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await within(library).findByText('Frieren');
    expect(within(library).getByTestId('library-list')).toBeInTheDocument();

    const tabs = within(library).getByRole('tablist', { name: 'List' });
    expect(
      within(tabs)
        .getAllByRole('tab')
        .map((tab) => tab.textContent),
    ).toEqual([
      'All 3',
      'Favorites 1',
      'Reading 1',
      'Completed 1',
      'On hold 0',
      'Dropped 0',
      'Plan to read 0',
    ]);
    await user.click(within(tabs).getByRole('tab', { name: /Favorites/ }));
    expect(items(library)).toEqual(['Berserk']);
    await user.click(within(tabs).getByRole('tab', { name: /All/ }));

    // The continue button opens where reading continues.
    await user.click(within(library).getByRole('button', { name: 'Continue reading Frieren' }));
    expect(commands.libraryContinue).toHaveBeenCalledWith('manga', 'a');
    await vi.waitFor(() => expect(commands.mangaChapter).toHaveBeenCalledWith('a', 'u7'));
  });

  it('changes the layout and opens a random title from the sheet and menu', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await within(library).findByText('Frieren');
    await user.click(within(library).getByRole('button', { name: 'Filter and sort' }));
    const sheet = screen.getByRole('dialog', { name: 'Filters' });
    await user.click(within(sheet).getByRole('tab', { name: 'Display' }));
    await user.click(within(sheet).getByRole('button', { name: 'Covers only' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'libraryDisplay',
      value: expect.objectContaining({ mode: 'coverOnlyGrid' }),
    });
    await user.click(within(sheet).getByRole('button', { name: 'Close' }));

    await user.click(within(library).getByRole('button', { name: 'More options' }));
    const menu = within(library).getByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Import comics' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Open a random title' }));
    await vi.waitFor(() => expect(commands.mangaTitle).toHaveBeenCalled());
  });
});

const TITLE: MangaTitleView = {
  id: 't9',
  hiddenNsfw: false,
  sourceId: 'dev.raygun.other/other',
  sourceName: 'Other',
  remoteId: 'x9',
  title: 'Frieren',
  coverUrl: null,
  authors: [],
  artists: [],
  description: null,
  genres: [],
  status: 'ongoing',
  url: null,
  inLibrary: false,
  chapters: [],
  continueChapterId: null,
};

describe('Title pages and the library', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaTitle.mockResolvedValue(ok(TITLE));
  });

  it('asks before adding a title that may already be in the library', async () => {
    const user = userEvent.setup();
    commands.libraryDuplicates.mockResolvedValue(
      ok([
        {
          titleId: 'a',
          sourceId: SOURCE,
          remoteId: 'a',
          title: "Frieren: Beyond Journey's End",
          sourceName: 'Comic Shelf',
          coverUrl: null,
        },
      ]),
    );
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(TITLE.sourceId)}&id=x9`);
    const page = await screen.findByTestId('manga-title-page');
    await user.click(await within(page).findByRole('button', { name: 'Add to library' }));
    expect(commands.libraryDuplicates).toHaveBeenCalledWith('manga', 't9', 'Frieren');
    const dialog = await screen.findByRole('dialog', { name: 'Already in your library?' });
    expect(
      within(dialog).getByRole('link', {
        name: "Open Frieren: Beyond Journey's End from Comic Shelf",
      }),
    ).toHaveAttribute('href', `/manga/title?source=${encodeURIComponent(SOURCE)}&id=a`);
    expect(commands.mangaSetInLibrary).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole('button', { name: 'Add anyway' }));
    expect(commands.mangaSetInLibrary).toHaveBeenCalledWith('t9', true);
  });

  it('puts a title on a list and among favorites', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(TITLE.sourceId)}&id=x9`);
    const page = await screen.findByTestId('manga-title-page');
    await user.click(await within(page).findByRole('button', { name: 'Add to a list' }));
    const sheet = await screen.findByRole('dialog', { name: 'List' });
    await user.click(within(sheet).getByRole('radio', { name: 'Plan to read' }));
    expect(commands.librarySetList).toHaveBeenCalledWith('manga', ['t9'], 'planned');
    await user.click(within(page).getByRole('button', { name: 'Favorite' }));
    expect(commands.librarySetFavorite).toHaveBeenCalledWith('manga', ['t9'], true);
  });
});
