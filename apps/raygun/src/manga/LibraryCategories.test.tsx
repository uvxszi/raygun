import type { MangaLibraryEntry } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const entry = (overrides: Partial<MangaLibraryEntry>): MangaLibraryEntry => libraryEntry(overrides);

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
    const names = { music: 'Audio', manga: 'Comics', novel: 'Books', video: 'Videos' };
    for (const [media, name] of Object.entries(names)) {
      expect(await screen.findByTestId(`library-media-${media}`)).toHaveTextContent(name);
    }
    // Podcasts are one of Audio's chips.
    expect(screen.getByTestId('library-view-podcasts')).toHaveTextContent('Podcasts');
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
    const sheet = screen.getByRole('dialog');
    // Filters cycle off → only → without (Mihon's tri-state).
    await user.click(within(sheet).getByRole('button', { name: 'Unread: off' }));
    expect(shownTitles(library)).toEqual(['Chainsaw Man', 'Frieren']);
    await user.click(within(sheet).getByRole('button', { name: 'Unread: only' }));
    expect(shownTitles(library)).toEqual(['Berserk']);
    await user.click(within(sheet).getByRole('button', { name: 'Unread: without' }));
    const tags = within(sheet).getByRole('group', { name: 'Tags' });
    await user.click(within(tags).getByRole('button', { name: 'Horror: off' }));
    expect(shownTitles(library)).toEqual(['Berserk', 'Chainsaw Man']);
    await user.click(within(tags).getByRole('button', { name: 'Fantasy: off' }));
    await user.click(within(tags).getByRole('button', { name: 'Fantasy: only' }));
    expect(shownTitles(library)).toEqual(['Chainsaw Man']);
    await user.click(within(sheet).getByRole('button', { name: 'Clear filters' }));

    // Sort: picking a sort keeps the direction; picking it again flips it.
    await user.click(within(sheet).getByRole('tab', { name: 'Sort' }));
    await user.click(within(sheet).getByRole('radio', { name: 'Unread count' }));
    expect(shownTitles(library)).toEqual(['Frieren', 'Chainsaw Man', 'Berserk']);
    await user.click(within(sheet).getByRole('radio', { name: /Unread count/ }));
    expect(shownTitles(library), 'nothing unread stays last').toEqual([
      'Chainsaw Man',
      'Frieren',
      'Berserk',
    ]);
    await user.click(within(sheet).getByRole('button', { name: 'Close' }));

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
