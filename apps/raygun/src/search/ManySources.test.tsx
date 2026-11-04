import type { MangaSource, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { useSourceHealth } from './sourceHealth';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const provider = (id: string, name: string) => ({
  id: `${id}/search`,
  extensionId: id,
  kind: 'music.metadata',
  name,
});

const track = (title: string): OnlineTrack => ({
  externalId: title,
  title,
  artist: 'Nina Simone',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
});

const FAST = provider('dev.test.fast', 'Fast');
const SLOW = provider('dev.test.slow', 'Slow');

const found = (from: typeof FAST, titles: string[]): ProviderSearchResult => ({
  provider: from,
  tracks: titles.map(track),
  error: null,
});

describe('Searching many sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    localStorage.setItem('raygun.search.grouped.music', '1');
    useSourceHealth.setState({ failures: {} });
  });

  it('shows what came in time, then the slow sources’ songs as they answer', async () => {
    commands.onlineSearch
      .mockResolvedValueOnce(
        ok({ library: [], top: [], providers: [found(FAST, ['Sinnerman'])], pending: [SLOW] }),
      )
      .mockResolvedValue(
        ok({
          library: [],
          top: [],
          providers: [found(FAST, ['Sinnerman']), found(SLOW, ['Feeling Good'])],
        }),
      );
    await AppShellWrapper.mount('/search?type=music&view=songs&q=nina');

    expect(await screen.findByText('Sinnerman')).toBeInTheDocument();
    expect(screen.getByText('1 of 2 sources answered')).toBeInTheDocument();
    // Asked again a moment later, the slow source's songs are in and nothing is pending.
    expect(await screen.findByText('Feeling Good', {}, { timeout: 4000 })).toBeInTheDocument();
    expect(screen.queryByText('1 of 2 sources answered')).not.toBeInTheDocument();
    expect(commands.onlineSearch).toHaveBeenCalledTimes(2);
  });

  it('keeps a search stopped for a newer one from marking its source as failing', async () => {
    const source: MangaSource = {
      id: 'x.royal/novel',
      name: 'Royal Road',
      extensionId: 'x.royal',
      enabled: true,
    };
    commands.novelSources.mockResolvedValue(ok([source]));
    commands.catalogGlobalSearch.mockResolvedValue(
      ok([{ source, items: [], hasNextPage: false, error: 'Stopped for a newer search' }]),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother');

    const row = await screen.findByTestId('source-row');
    expect(await within(row).findByTestId('source-issue')).toHaveTextContent(
      'Stopped for a newer search',
    );
    expect(useSourceHealth.getState().failures).toEqual({});
    // Not asked again over and over: a stopped search isn't retried.
    expect(commands.catalogGlobalSearch).toHaveBeenCalledTimes(1);
  });
});
