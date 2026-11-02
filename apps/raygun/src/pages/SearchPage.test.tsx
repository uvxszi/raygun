import type { OnlineTrack, ProviderSearchResult, SearchResults } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const PROVIDER = {
  id: 'dev.raygun.internet-archive/search',
  extensionId: 'dev.raygun.internet-archive',
  kind: 'music.metadata',
  name: 'Internet Archive',
};

const track = (title: string, playable = true): OnlineTrack => ({
  externalId: title,
  title,
  artist: 'Louis Armstrong',
  album: 'Hot Fives',
  durationMs: 185_000,
  artworkUrl: null,
  source: playable ? { kind: 'url', url: `https://archive.org/${title}.mp3`, headers: {} } : null,
});

const answer = (
  providers: ProviderSearchResult[],
  extra: Partial<Omit<SearchResults, 'providers'>> = {},
) =>
  bindingsMock.commands.onlineSearch.mockResolvedValue(
    ok({ library: [], top: [], providers, ...extra }),
  );

describe('Search', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it("shows what's popular before anything is typed", async () => {
    await AppShellWrapper.mount('/search');

    expect(
      await screen.findByText('Popular now. Type above to search everything.'),
    ).toBeInTheDocument();
    expect(bindingsMock.commands.onlineSearch).not.toHaveBeenCalled();
  });

  it('searches once typing pauses and groups results by source', async () => {
    answer([
      {
        provider: PROVIDER,
        tracks: [track('West End Blues'), track('Potato Head Blues')],
        error: null,
      },
    ]);
    await AppShellWrapper.mount('/search?type=music');

    await userEvent.type(screen.getByRole('searchbox', { name: 'Search' }), 'armstrong');

    const group = await screen.findByTestId('provider-results');
    expect(group).toHaveTextContent('Internet Archive');
    expect(within(group).getAllByTestId('online-track-row')).toHaveLength(2);
    expect(group).toHaveTextContent('Louis Armstrong · Hot Fives');
    expect(bindingsMock.commands.onlineSearch).toHaveBeenCalledTimes(1);
    expect(bindingsMock.commands.onlineSearch).toHaveBeenCalledWith('armstrong');
  });

  it('plays results from the tapped track, finding streams for those without one', async () => {
    answer([
      {
        provider: PROVIDER,
        tracks: [track('One'), track('No source', false), track('Two')],
        error: null,
      },
    ]);
    await AppShellWrapper.mount('/search?type=music&q=blues');

    const rows = await screen.findAllByTestId('online-track-row');
    await userEvent.click(rows[1]!);

    const [items, start] = bindingsMock.commands.playbackReplace.mock.calls[0]!;
    expect(items.map((item: { title: string }) => item.title)).toEqual(['One', 'No source', 'Two']);
    expect(items[0]).toMatchObject({
      artist: 'Louis Armstrong',
      mediaKind: 'audio',
      source: { kind: 'url', url: 'https://archive.org/One.mp3' },
    });
    expect(items[1].source).toEqual({
      kind: 'lookup',
      title: 'No source',
      artist: 'Louis Armstrong',
      album: 'Hot Fives',
      durationMs: 185_000,
    });
    expect(start).toBe(1);
  });

  it('shows a failing source next to working ones', async () => {
    answer([
      { provider: PROVIDER, tracks: [track('One')], error: null },
      {
        provider: { ...PROVIDER, id: 'other/search', name: 'Other' },
        tracks: [],
        error: 'offline',
      },
    ]);
    await AppShellWrapper.mount('/search?type=music&q=blues');

    expect(await screen.findByRole('alert')).toHaveTextContent('This source failed: offline');
    expect(screen.getAllByTestId('online-track-row')).toHaveLength(1);
  });

  it('names the extension when two sources share a name', async () => {
    answer([
      { provider: PROVIDER, tracks: [track('One')], error: null },
      {
        provider: { ...PROVIDER, id: 'nuclear.ia/search', extensionId: 'nuclear.ia' },
        tracks: [track('Two')],
        error: null,
      },
    ]);
    await AppShellWrapper.mount('/search?type=music&q=blues');

    const headings = await screen.findAllByRole('heading', { level: 3 });
    expect(headings.map((heading) => heading.textContent)).toEqual([
      `${PROVIDER.name} · dev.raygun.internet-archive`,
      `${PROVIDER.name} · nuclear.ia`,
    ]);
  });

  it('says when nothing was found, or no sources exist', async () => {
    answer([{ provider: PROVIDER, tracks: [], error: null }]);
    await AppShellWrapper.mount('/search?type=music&q=zzzz');
    expect(await screen.findByText('Nothing found for “zzzz”.')).toBeInTheDocument();
  });

  it('says when no search sources are installed', async () => {
    answer([]);
    await AppShellWrapper.mount('/search?type=music&q=zzzz');
    expect(await screen.findByText('No search sources are installed.')).toBeInTheDocument();
  });
});
