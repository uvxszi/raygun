import type { MangaLibraryEntry } from '@raygun/bindings';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { HOLD_MS } from '../select/scopeContext';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.example.comics/comics';

const entry = (overrides: Partial<MangaLibraryEntry>): MangaLibraryEntry =>
  libraryEntry({ sourceId: SOURCE, ...overrides });

// Listed by title: Berserk, Chainsaw Man, Frieren.
const LIBRARY = [
  entry({ titleId: 'a', remoteId: 'a', title: 'Frieren', unread: 3, categoryIds: ['c1'] }),
  entry({ titleId: 'b', remoteId: 'b', title: 'Berserk' }),
  entry({ titleId: 'c', remoteId: 'c', title: 'Chainsaw Man', unread: 9 }),
];
const CATEGORIES = [
  { id: 'c1', name: 'Reading', display: null },
  { id: 'c2', name: 'Later', display: null },
];

const heading = (library: HTMLElement) => within(library).queryByRole('heading', { level: 1 });

const mount = async (medium = 'manga') => {
  await AppShellWrapper.mount(`/library?media=${medium}`);
  const library = await screen.findByTestId(`${medium}-library`);
  await within(library).findByText('Frieren');
  return library;
};

describe('Picking titles in the library', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaLibrary.mockResolvedValue(ok(LIBRARY));
    commands.novelLibrary.mockResolvedValue(ok(LIBRARY));
    commands.libraryCategories.mockResolvedValue(ok(CATEGORIES));
  });

  it('puts the circle on the cover’s top left and the ⋮ on its top right', async () => {
    const library = await mount();
    const card = within(library)
      .getAllByTestId('library-item')
      .find((item) => item.getAttribute('aria-label') === 'Frieren')!;
    const circle = screen.getByRole('checkbox', { name: 'Select Frieren' });
    const options = within(card).getByRole('button', { name: 'Options for Frieren' });
    expect(circle.className).toMatch(/\btop-1\.5\b.*\bleft-1\.5\b/);
    expect(circle.className).not.toMatch(/\bright-1\.5\b/);
    expect(options.className).toMatch(/\btop-1\.5\b.*\bright-1\.5\b/);
  });

  it('picks with the circle, Ctrl and Shift clicks, selects all, and ends on Escape', async () => {
    const user = userEvent.setup();
    const library = await mount();

    await user.click(screen.getByRole('checkbox', { name: 'Select Berserk' }));
    expect(heading(library)).toHaveTextContent('1 selected');
    expect(screen.getByRole('checkbox', { name: 'Select Berserk' })).toHaveAttribute(
      'aria-checked',
      'true',
    );

    // Ctrl-click adds one without opening it.
    await user.keyboard('{Control>}');
    await user.click(within(library).getByRole('link', { name: 'Frieren' }));
    await user.keyboard('{/Control}');
    expect(heading(library)).toHaveTextContent('2 selected');
    expect(screen.getByTestId('manga-library')).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await waitFor(() => expect(heading(library)).not.toHaveTextContent(/selected/));

    // Shift-click adds everything since the last pick.
    await user.click(screen.getByRole('checkbox', { name: 'Select Berserk' }));
    await user.keyboard('{Shift>}');
    await user.click(within(library).getByRole('link', { name: 'Frieren' }));
    await user.keyboard('{/Shift}');
    expect(heading(library)).toHaveTextContent('3 selected');

    // While picking, a tap takes one out; Select all puts it back.
    await user.click(within(library).getByRole('link', { name: 'Chainsaw Man' }));
    expect(heading(library)).toHaveTextContent('2 selected');
    await user.click(within(library).getByRole('button', { name: 'Select all' }));
    expect(heading(library)).toHaveTextContent('3 selected');

    await user.click(
      within(within(library).getByRole('toolbar', { name: 'Selected titles' })).getByRole(
        'button',
        { name: 'Add to favorites' },
      ),
    );
    expect(commands.librarySetFavorite).toHaveBeenCalledWith(
      'manga',
      expect.arrayContaining(['a', 'b', 'c']),
      true,
    );
  });

  it('picks with a finger held on a cover, without opening it', async () => {
    const user = userEvent.setup();
    const library = await mount();
    const target = within(library).getByRole('link', { name: 'Chainsaw Man' });
    await user.pointer({ keys: '[TouchA>]', target });
    await act(() => new Promise((resolve) => setTimeout(resolve, HOLD_MS + 50)));
    await user.pointer({ keys: '[/TouchA]', target });
    expect(heading(library)).toHaveTextContent('1 selected');
    expect(screen.getByRole('checkbox', { name: 'Select Chainsaw Man' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(screen.getByTestId('manga-library')).toBeInTheDocument();
  });

  it('opens a title’s ⋮, also while picking, and sets its groups from it', async () => {
    const user = userEvent.setup();
    const library = await mount('novel');

    await user.click(within(library).getByRole('button', { name: 'Options for Frieren' }));
    const menu = screen.getByRole('menu');
    const names = within(menu)
      .getAllByRole('menuitem')
      .map((item) => item.textContent);
    expect(names).toEqual(
      expect.arrayContaining([
        'Select',
        'Read',
        'Open page',
        'Remove from library',
        'Mark as read',
        'Set groups',
      ]),
    );
    await user.click(within(menu).getByRole('menuitem', { name: 'Set groups' }));
    const sheet = await screen.findByRole('dialog', { name: 'Set groups' });
    expect(within(sheet).getByRole('checkbox', { name: 'Reading' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.click(within(sheet).getByRole('checkbox', { name: 'Later' }));
    await user.click(within(sheet).getByRole('button', { name: 'OK' }));
    expect(commands.libraryChangeCategories).toHaveBeenCalledWith('novel', ['a'], ['c1', 'c2'], []);

    // Picking: the ⋮ still opens the title's menu, and its Select picks it.
    await user.click(screen.getByRole('checkbox', { name: 'Select Berserk' }));
    await user.click(within(library).getByRole('button', { name: 'Options for Chainsaw Man' }));
    expect(heading(library)).toHaveTextContent('1 selected');
    await user.click(within(screen.getByRole('menu')).getByRole('menuitem', { name: 'Select' }));
    await waitFor(() => expect(heading(library)).toHaveTextContent('2 selected'));

    await user.click(within(library).getByRole('button', { name: 'Options for Frieren' }));
    await user.click(
      within(screen.getByRole('menu')).getByRole('menuitem', { name: 'Mark as read' }),
    );
    await waitFor(() =>
      expect(commands.libraryMarkRead).toHaveBeenCalledWith('novel', ['a'], true),
    );
  });
});
