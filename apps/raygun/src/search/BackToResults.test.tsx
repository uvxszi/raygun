import type { StremioItem, StremioMeta } from '@raygun/bindings';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { forgetLists } from '../shell/listMemory';
import { forgetScrollPositions } from '../shell/scrollMemory';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const CARD_PX = 100;
const VIEW_PX = 600;

/** jsdom has no layout: `<main>` scrolls as far as its cards reach. */
const giveLayout = (main: HTMLElement) => {
  let top = 0;
  const height = () => main.querySelectorAll('[data-testid="stremio-card"]').length * CARD_PX;
  Object.defineProperty(main, 'clientHeight', { configurable: true, get: () => VIEW_PX });
  Object.defineProperty(main, 'scrollHeight', { configurable: true, get: height });
  Object.defineProperty(main, 'scrollTop', {
    configurable: true,
    get: () => top,
    set: (value: number) => {
      top = Math.max(0, Math.min(value, height() - VIEW_PX));
    },
  });
  main.scrollTo = ((_left: number, value: number) => {
    main.scrollTop = value;
  }) as typeof main.scrollTo;
};

const movie = (index: number): StremioItem => ({
  kind: 'movie',
  id: `tt${1000 + index}`,
  name: `Sintel ${index}`,
  poster: null,
  posterShape: 'poster',
  releaseInfo: '2010',
  rating: null,
  playable: true,
});

const MOVIES = Array.from({ length: 60 }, (_, index) => movie(index + 1));

const META: StremioMeta = {
  continueVideoId: null,
  kind: 'movie',
  id: 'tt1001',
  name: 'Sintel 1',
  poster: null,
  background: null,
  logo: null,
  description: null,
  genres: [],
  releaseInfo: '2010',
  runtime: null,
  rating: null,
  ratings: [],
  cast: [],
  director: [],
  trailerYtIds: [],
  videos: [],
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded: false,
  addonName: 'Cinemeta',
  addonId: 'cinemeta',
  videoId: 'tt1001',
  live: false,
  inLibrary: false,
  titleId: null,
};

describe('Back to search results', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    forgetLists();
    forgetScrollPositions();
    commands.stremioCatalogs.mockResolvedValue(
      ok([
        {
          addonId: 'cinemeta',
          addonName: 'Cinemeta',
          kind: 'movie',
          catalogId: 'top',
          name: 'Popular',
          extras: [],
          board: false,
          searchable: true,
          discoverable: true,
          pages: true,
          failing: false,
        },
      ]),
    );
    commands.stremioSearchCatalog.mockResolvedValue(
      ok({
        addonId: 'cinemeta',
        addonName: 'Cinemeta',
        kind: 'movie',
        catalogId: 'top',
        name: 'Popular',
        items: MOVIES,
        genres: [],
        hasMore: false,
        error: null,
      }),
    );
    commands.stremioMeta.mockResolvedValue(ok(META));
  });

  it('keeps what Show more loaded and the place scrolled to', async () => {
    const user = userEvent.setup();
    const router = await AppShellWrapper.mount(
      '/browse?tab=search&type=video&view=movies&q=sintel',
    );
    const main = screen.getByTestId('page-content');
    giveLayout(main);
    const section = await screen.findByRole('region', { name: 'Movies' });
    await waitFor(() => expect(within(section).getAllByTestId('stremio-card')).toHaveLength(48));

    await user.click(within(section).getByRole('button', { name: 'Show 12 more' }));
    await waitFor(() => expect(within(section).getAllByTestId('stremio-card')).toHaveLength(60));
    act(() => {
      main.scrollTop = 4000;
    });

    await user.click(within(section).getByRole('link', { name: 'Sintel 50' }));
    await screen.findByTestId('watch-title-page');

    act(() => router.history.back());
    const again = await screen.findByRole('region', { name: 'Movies' });
    // Every title Show more had added is there again, without pressing it.
    await waitFor(() => expect(within(again).getAllByTestId('stremio-card')).toHaveLength(60));
    expect(
      within(again).queryByRole('button', { name: /^Show \d+ more$/ }),
    ).not.toBeInTheDocument();
    await waitFor(() => expect(main.scrollTop).toBe(4000));
  });

  it('starts a new search afresh', async () => {
    const user = userEvent.setup();
    const router = await AppShellWrapper.mount(
      '/browse?tab=search&type=video&view=movies&q=sintel',
    );
    const section = await screen.findByRole('region', { name: 'Movies' });
    await user.click(await within(section).findByRole('button', { name: 'Show 12 more' }));
    await waitFor(() => expect(within(section).getAllByTestId('stremio-card')).toHaveLength(60));

    await act(() =>
      router.navigate({
        to: '/browse',
        search: { tab: 'search', type: 'video', view: 'movies', q: 'sintel 2' },
      }),
    );
    const fresh = await screen.findByRole('region', { name: 'Movies' });
    await waitFor(() => expect(within(fresh).getAllByTestId('stremio-card')).toHaveLength(48));
  });
});
