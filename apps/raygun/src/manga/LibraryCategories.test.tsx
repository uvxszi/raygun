import type { MangaLibraryEntry } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const entry = (overrides: Partial<MangaLibraryEntry>): MangaLibraryEntry => ({
  titleId: 't',
  sourceId: 'src',
  remoteId: 'r',
  title: 'Title',
  coverUrl: null,
  chapters: 10,
  unread: 0,
  genres: [],
  status: 'ongoing',
  addedAt: 0,
  lastReadAt: null,
  latestChapterAt: null,
  started: false,
  downloaded: 0,
  categoryIds: [],
  ...overrides,
});

const LIBRARY = [
  entry({
    titleId: 'a',
    remoteId: 'a',
    title: 'Frieren',
    unread: 3,
    genres: ['Fantasy'],
    categoryIds: ['c1'],
  }),
  entry({ titleId: 'b', remoteId: 'b', title: 'Berserk', genres: ['Fantasy', 'Horror'] }),
  entry({
    titleId: 'c',
    remoteId: 'c',
    title: 'Chainsaw Man',
    unread: 9,
    genres: ['Horror'],
    categoryIds: ['c2'],
  }),
];
const CATEGORIES = [
  { id: 'c1', name: 'Reading' },
  { id: 'c2', name: 'Later' },
];

const TITLES = ['Frieren', 'Berserk', 'Chainsaw Man'];
/** The titles of the cards shown, in order (card text also holds the unread badge). */
const shownTitles = (container: HTMLElement) =>
  within(container)
    .queryAllByRole('link')
    .map((link) => TITLES.find((title) => (link.textContent ?? '').includes(title)))
    .filter((title): title is string => title !== undefined);

describe('Library categories and filters', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaLibrary.mockResolvedValue(ok(LIBRARY));
    commands.libraryCategories.mockResolvedValue(ok(CATEGORIES));
  });

  it('shows every media type and splits the manga library into category tabs', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library');
    for (const media of ['music', 'podcast', 'manga', 'novel', 'video']) {
      expect(await screen.findByTestId(`library-media-${media}`)).toBeInTheDocument();
    }
    await user.click(screen.getByTestId('library-media-manga'));
    const library = await screen.findByTestId('manga-library');
    await within(library).findByText('Frieren');

    const tabs = within(library).getByRole('tablist', { name: 'Categories' });
    expect(
      within(tabs)
        .getAllByRole('tab')
        .map((tab) => tab.textContent),
    ).toEqual(['All 3', 'Reading 1', 'Later 1', 'Uncategorized 1']);
    await user.click(within(tabs).getByRole('tab', { name: /Reading/ }));
    expect(shownTitles(library)).toEqual(['Frieren']);
    await user.click(within(tabs).getByRole('tab', { name: /Uncategorized/ }));
    expect(shownTitles(library)).toEqual(['Berserk']);
  });

  it('filters by state and tags, searches and sorts', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await within(library).findByText('Frieren');
    expect(shownTitles(library)).toEqual(['Berserk', 'Chainsaw Man', 'Frieren']);

    await user.click(within(library).getByRole('button', { name: 'Filter and sort' }));
    await user.click(within(library).getByRole('button', { name: 'Unread' }));
    expect(shownTitles(library)).toEqual(['Chainsaw Man', 'Frieren']);
    const tags = within(library).getByRole('group', { name: 'Tags' });
    await user.click(within(tags).getByRole('button', { name: 'Horror' }));
    expect(shownTitles(library)).toEqual(['Chainsaw Man']);
    await user.click(within(library).getByRole('button', { name: 'Clear filters' }));

    await user.selectOptions(within(library).getByRole('combobox', { name: 'Sort by' }), 'unread');
    await user.click(within(library).getByRole('button', { name: 'Ascending' }));
    expect(shownTitles(library)).toEqual(['Chainsaw Man', 'Frieren', 'Berserk']);
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Close' }));

    // Searching is the top bar's: the title turns into a search field.
    await user.click(within(library).getByRole('button', { name: 'Search the library' }));

    await user.type(within(library).getByRole('searchbox', { name: /Search your library/ }), 'fri');
    expect(shownTitles(library)).toEqual(['Frieren']);
    await user.type(within(library).getByRole('searchbox', { name: /Search your library/ }), 'zzz');
    expect(within(library).getByText('Nothing in the library matches.')).toBeInTheDocument();
  });

  it('adds, renames, moves and deletes categories', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library/categories?medium=manga');
    const page = await screen.findByTestId('categories-page');
    await within(page).findByRole('listitem', { name: 'Reading' });

    await user.click(within(page).getByRole('button', { name: 'New category' }));
    const sheet = await screen.findByRole('dialog', { name: 'New category' });
    await user.type(within(sheet).getByRole('textbox', { name: 'New category name' }), 'Done');
    await user.click(within(sheet).getByRole('button', { name: 'New category' }));
    expect(commands.libraryCategoryCreate).toHaveBeenCalledWith('manga', 'Done');

    await user.click(within(page).getByRole('button', { name: 'Move Later up' }));
    expect(commands.libraryCategoryReorder).toHaveBeenCalledWith('manga', ['c2', 'c1']);

    await user.click(within(page).getByRole('button', { name: 'Rename Reading' }));
    const input = within(page).getByRole('textbox', { name: 'Category name' });
    await user.clear(input);
    await user.type(input, 'Reading now{Enter}');
    expect(commands.libraryCategoryRename).toHaveBeenCalledWith('manga', 'c1', 'Reading now');

    await user.click(within(page).getByRole('button', { name: 'Delete Later' }));
    expect(commands.libraryCategoryDelete).toHaveBeenCalledWith('manga', 'c2');
  });
});
