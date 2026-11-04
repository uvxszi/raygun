import type { OnlineTrack } from '@raygun/bindings';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { HOLD_MS } from './scopeContext';
import { pickedKinds, useSelectionStore } from './selectionStore';
import type { Picked } from './selectionStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.raygun.mangadex/mangadex';

const card = (id: string, title: string) => ({
  sourceId: SOURCE,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
});

const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
});

const DEEZER = {
  id: 'dev.raygun.deezer/catalogue',
  extensionId: 'dev.raygun.deezer',
  kind: 'music.metadata',
  name: 'Deezer',
};

const mountSource = async () => {
  await AppShellWrapper.mount(`/manga/source?id=${encodeURIComponent(SOURCE)}`);
  const page = await screen.findByTestId('manga-source-page');
  await within(page).findByText('Frieren');
  return page;
};

const pickable = (page: HTMLElement, title: string) =>
  within(page)
    .getAllByTestId('selectable')
    .find((element) => within(element).queryByText(title))!;

const bar = () => screen.getByTestId('search-selection-bar');

describe('Picking search and source results', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    sessionStorage.clear();
    localStorage.clear();
    useSelectionStore.getState().clear();
    commands.mangaSources.mockResolvedValue(
      ok([{ id: SOURCE, name: 'MangaDex', extensionId: 'dev.raygun.mangadex', enabled: true }]),
    );
    commands.mangaBrowse.mockResolvedValue(
      ok({
        items: [card('f', 'Frieren'), card('d', 'Dandadan'), card('k', 'Kaiju No. 8')],
        hasNextPage: false,
      }),
    );
  });

  it('starts with Ctrl-click, then taps add and take out, and adds them to the library', async () => {
    const user = userEvent.setup();
    const page = await mountSource();
    expect(screen.queryByTestId('search-selection-bar')).not.toBeInTheDocument();

    await user.keyboard('{Control>}');
    await user.click(within(pickable(page, 'Frieren')).getByText('Frieren'));
    await user.keyboard('{/Control}');
    expect(pickable(page, 'Frieren')).toHaveAttribute('data-selected', 'true');
    expect(bar()).toHaveTextContent('1 selected');
    // The page stayed: the click picked instead of opening.
    expect(screen.getByTestId('manga-source-page')).toBeInTheDocument();

    // While picking, a plain tap adds and takes out.
    await user.click(within(pickable(page, 'Dandadan')).getByText('Dandadan'));
    await user.click(within(pickable(page, 'Kaiju No. 8')).getByText('Kaiju No. 8'));
    await user.click(within(pickable(page, 'Kaiju No. 8')).getByText('Kaiju No. 8'));
    expect(bar()).toHaveTextContent('2 selected');
    expect(pickable(page, 'Kaiju No. 8')).not.toHaveAttribute('data-selected');

    commands.libraryAddTitles.mockResolvedValueOnce(
      ok({ titleIds: ['t1', 't2'], queued: 0, failed: [] }),
    );
    await user.click(within(bar()).getByRole('button', { name: 'Add to library' }));
    await waitFor(() =>
      expect(commands.libraryAddTitles).toHaveBeenCalledWith(
        'manga',
        [
          { sourceId: SOURCE, remoteId: 'f', title: 'Frieren' },
          { sourceId: SOURCE, remoteId: 'd', title: 'Dandadan' },
        ],
        [],
      ),
    );
    expect(await screen.findByText('2 added to the library')).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByTestId('search-selection-bar')).not.toBeInTheDocument(),
    );
  });

  it('picks on a long press without opening the card or its menu', async () => {
    const user = userEvent.setup();
    const page = await mountSource();
    const target = within(pickable(page, 'Frieren')).getByText('Frieren');
    await user.pointer({ keys: '[TouchA>]', target });
    await act(() => new Promise((resolve) => setTimeout(resolve, HOLD_MS + 50)));
    await user.pointer({ keys: '[/TouchA]', target });
    expect(pickable(page, 'Frieren')).toHaveAttribute('data-selected', 'true');
    expect(bar()).toHaveTextContent('1 selected');
    expect(screen.getByTestId('manga-source-page')).toBeInTheDocument();
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('clears on Escape and with the bar’s Clear', async () => {
    const user = userEvent.setup();
    const page = await mountSource();
    await user.click(
      within(pickable(page, 'Frieren')).getByRole('checkbox', { name: 'Select Frieren' }),
    );
    expect(bar()).toHaveTextContent('1 selected');
    await user.keyboard('{Escape}');
    await waitFor(() =>
      expect(screen.queryByTestId('search-selection-bar')).not.toBeInTheDocument(),
    );
    expect(pickable(page, 'Frieren')).not.toHaveAttribute('data-selected');

    await user.click(
      within(pickable(page, 'Dandadan')).getByRole('checkbox', { name: 'Select Dandadan' }),
    );
    await user.click(within(bar()).getByRole('button', { name: 'Clear selection' }));
    expect(screen.queryByTestId('search-selection-bar')).not.toBeInTheDocument();
  });

  it('adds picked comics to a group, and downloads their next chapters', async () => {
    const user = userEvent.setup();
    commands.libraryCategories.mockResolvedValue(
      ok([{ id: 'g1', name: 'Shonen', position: 0, display: null }]),
    );
    const page = await mountSource();
    await user.click(
      within(pickable(page, 'Frieren')).getByRole('checkbox', { name: 'Select Frieren' }),
    );
    await user.click(
      within(pickable(page, 'Dandadan')).getByRole('checkbox', { name: 'Select Dandadan' }),
    );

    await user.click(within(bar()).getByRole('button', { name: 'Add to group' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add to group' });
    const add = within(sheet).getByRole('button', { name: 'Add to group' });
    expect(add).toBeDisabled();
    await user.click(await within(sheet).findByRole('checkbox', { name: 'Shonen' }));
    commands.libraryAddTitles.mockResolvedValueOnce(
      ok({ titleIds: ['t1', 't2'], queued: 0, failed: [] }),
    );
    await user.click(add);
    await waitFor(() =>
      expect(commands.libraryAddTitles).toHaveBeenCalledWith('manga', expect.any(Array), ['g1']),
    );
    expect(await screen.findByText('2 added to Shonen')).toBeInTheDocument();

    await user.click(
      within(pickable(page, 'Kaiju No. 8')).getByRole('checkbox', { name: 'Select Kaiju No. 8' }),
    );
    await user.click(within(bar()).getByRole('button', { name: 'Download all' }));
    const download = await screen.findByRole('dialog', { name: 'Download' });
    commands.libraryDownloadTitles.mockResolvedValueOnce(
      ok({
        titleIds: ['t3'],
        queued: 5,
        failed: [{ title: 'Kaiju No. 8', message: 'The source didn’t answer in time' }],
      }),
    );
    await user.click(within(download).getByRole('button', { name: 'Next 5 chapters' }));
    await waitFor(() =>
      expect(commands.libraryDownloadTitles).toHaveBeenCalledWith(
        'manga',
        [{ sourceId: SOURCE, remoteId: 'k', title: 'Kaiju No. 8' }],
        5,
      ),
    );
    expect(
      await screen.findByText('5 downloads queued · Kaiju No. 8: The source didn’t answer in time'),
    ).toBeInTheDocument();
  });

  it('offers songs their own actions, and a mix only what its kinds share', async () => {
    const user = userEvent.setup();
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: DEEZER,
            tracks: [song('Get Lucky', 'Daft Punk'), song('One More Time', 'Daft Punk')],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=daft%20punk');
    const best = await screen.findByTestId('best-songs');
    const rows = within(best).getAllByTestId('selectable');
    await user.click(within(rows[0]!).getByRole('checkbox', { name: 'Select Get Lucky' }));
    await user.click(within(rows[1]!).getByTestId('online-track-row'));
    expect(bar()).toHaveTextContent('2 selected');
    const names = within(bar())
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-label'));
    expect(names).toEqual(['Clear selection', 'Favorite', 'Add to playlist', 'Download all']);

    await user.click(within(bar()).getByRole('button', { name: 'Favorite' }));
    await waitFor(() =>
      expect(commands.favoritesSetMany).toHaveBeenCalledWith(
        [
          expect.objectContaining({ title: 'Get Lucky', artist: 'Daft Punk' }),
          expect.objectContaining({ title: 'One More Time', artist: 'Daft Punk' }),
        ],
        true,
      ),
    );
    expect(await screen.findByText('2 added to favorites')).toBeInTheDocument();

    // A song with a comic: favorite and download only.
    act(() => {
      useSelectionStore.getState().toggle({ kind: 'song', track: song('Get Lucky', 'Daft Punk') });
      useSelectionStore.getState().toggle({
        kind: 'title',
        medium: 'manga',
        sourceId: SOURCE,
        remoteId: 'f',
        title: 'Frieren',
      });
    });
    const mixed = within(bar())
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-label'));
    expect(mixed).toEqual(['Clear selection', 'Favorite', 'Download all']);
  });

  it('sorts what is picked by kind', () => {
    const title = (medium: 'manga' | 'novel' | 'video', id: string): Picked => ({
      kind: 'title',
      medium,
      sourceId: SOURCE,
      remoteId: id,
      title: id,
    });
    expect(pickedKinds([title('manga', 'a'), title('manga', 'b')]).medium).toBe('manga');
    const mixed = pickedKinds([
      title('manga', 'a'),
      title('novel', 'b'),
      { kind: 'song', track: song('x', 'y') },
    ]);
    expect(mixed.medium).toBeNull();
    expect(mixed.songs).toHaveLength(1);
    expect(mixed.titles).toHaveLength(2);
  });
});
