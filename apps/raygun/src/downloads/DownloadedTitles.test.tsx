import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SOURCE = 'dev.example.comics/comics';

describe('Downloads → Comics and Books', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.libraryRemove.mockResolvedValue(ok(null) as never);
  });

  it("lists every title with downloads, in the library or not, with the Library's menu", async () => {
    const user = userEvent.setup();
    commands.libraryDownloaded.mockImplementation(async (medium: string) =>
      ok(
        medium === 'manga'
          ? [
              libraryEntry({
                titleId: 't1',
                sourceId: SOURCE,
                remoteId: 'm1',
                title: 'Frieren',
                downloaded: 3,
                addedAt: 0,
              }),
            ]
          : [],
      ),
    );
    await AppShellWrapper.mount('/downloads?tab=comics');
    expect(commands.libraryDownloaded).toHaveBeenCalledWith('manga');
    const list = await screen.findByTestId('downloaded-manga');
    const row = within(list).getByRole('link', { name: 'Frieren' });
    expect(row).toHaveAttribute('href', expect.stringContaining('/manga/title'));
    expect(within(row).getByLabelText('3 chapters downloaded')).toBeInTheDocument();

    await user.click(within(list).getByRole('button', { name: 'Options for Frieren' }));
    const menu = await screen.findByRole('menu');
    // Kept out of the library: it can be added; and the Library's own options are there.
    for (const name of ['Read', 'Open page', 'Add to library', 'Mark as read', 'Export…']) {
      expect(within(menu).getByRole('menuitem', { name })).toBeInTheDocument();
    }
    expect(within(menu).getByRole('menuitem', { name: 'Copy title' })).toBeInTheDocument();

    // Deleting its downloads asks first, in the menu's place.
    await user.click(within(menu).getByRole('menuitem', { name: 'Delete downloaded chapters' }));
    expect(commands.libraryRemove).not.toHaveBeenCalled();
    const question = within(menu).getByRole('alertdialog', {
      name: 'Delete the downloaded chapters of “Frieren”?',
    });
    await user.click(within(question).getByRole('button', { name: 'Delete downloaded chapters' }));
    await vi.waitFor(() =>
      expect(commands.libraryRemove).toHaveBeenCalledWith('manga', ['t1'], false, true),
    );
  });

  it('exports and marks titles from the selection, as in the Library', async () => {
    const user = userEvent.setup();
    commands.libraryDownloaded.mockImplementation(async (medium: string) =>
      ok(
        medium === 'novel'
          ? [
              libraryEntry({ titleId: 'b1', title: 'Moby Dick', downloaded: 2, addedAt: 1 }),
              libraryEntry({ titleId: 'b2', title: 'Emma', downloaded: 1, addedAt: 1 }),
            ]
          : [],
      ),
    );
    await AppShellWrapper.mount('/downloads?tab=books');
    const list = await screen.findByTestId('downloaded-novel');
    await user.click(within(list).getByRole('button', { name: 'Options for Emma' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Select' }),
    );
    const actions = await screen.findByTestId('selection-actions');
    await user.click(within(actions).getByRole('button', { name: 'Export' }));
    expect(await screen.findByRole('dialog', { name: 'Export' })).toBeInTheDocument();
  });

  it('shows that medium only of the shared chapter queue', async () => {
    commands.mangaDownloads.mockResolvedValue(
      ok({
        paused: false,
        downloaded: 0,
        items: [
          {
            chapterId: 'c1',
            titleId: 't1',
            title: 'Frieren',
            chapter: 'Chapter 1',
            medium: 'manga',
            download: { state: 'queued', pagesDone: 0, pagesTotal: null, error: null },
          },
          {
            chapterId: 'c2',
            titleId: 'b1',
            title: 'Moby Dick',
            chapter: 'Loomings',
            medium: 'novel',
            download: { state: 'queued', pagesDone: 0, pagesTotal: null, error: null },
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/downloads?tab=books');
    const rows = await screen.findAllByTestId('download-row');
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent('Moby Dick');
  });
});
