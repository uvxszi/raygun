import type { PlayContext, SavedTrack } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { matchesReveal, songReveal } from '../music/reveal';
import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const saved = (title: string): SavedTrack => ({
  trackKey: `nina simone|${title.toLowerCase()}`,
  title,
  artist: 'Nina Simone',
  album: null,
  durationMs: 180_000,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt: null,
  playCount: 0,
});

const FAVORITES: PlayContext = {
  kind: 'favorites',
  name: 'Favorites',
  subtitle: null,
  artworkUrl: null,
  to: '/library',
  search: { view: 'favorites' },
  playedAt: 0,
};

const QUEUE = [makeItem('Sinnerman', 'Nina Simone'), makeItem('Feeling Good', 'Nina Simone')];

describe('Playing from', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('is hidden while the queue was played from nowhere in particular', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;

    expect(screen.queryByRole('button', { name: /^Playing from/ })).not.toBeInTheDocument();
  });

  it('opens the page the queue came from and marks the playing song there', async () => {
    commands.favoritesList.mockResolvedValue(
      ok([saved('Feeling Good'), saved('Sinnerman'), saved('Wild Is the Wind')]),
    );
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0, 'playing', FAVORITES);
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;

    await userEvent.click(within(sheet).getByRole('button', { name: 'Playing from Favorites' }));

    expect(await screen.findByTestId('favorites-view')).toBeInTheDocument();
    await waitFor(() => {
      const marked = document.querySelector('[data-revealed]');
      expect(marked).toHaveTextContent('Sinnerman');
    });
    expect(screen.queryByRole('dialog', { name: 'Now playing' })).not.toBeInTheDocument();
  });

  it('shows in the bar and the side panel on wide windows', async () => {
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: true,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    try {
      const user = userEvent.setup();
      await AppShellWrapper.mount();
      AppShellWrapper.playing(QUEUE, 0, 'playing', { ...FAVORITES, kind: 'album', name: 'Pastel' });
      await AppShellWrapper.miniPlayer.title;
      const bar = AppShellWrapper.miniPlayer.element;
      expect(within(bar).getByRole('button', { name: 'Playing from Pastel' })).toBeInTheDocument();

      await user.click(within(bar).getByRole('button', { name: 'Now playing view' }));
      const panel = screen.getByRole('complementary', { name: 'Now playing' });
      expect(within(panel).getByRole('button', { name: 'Playing from Pastel' })).toHaveTextContent(
        'Playing fromPastel',
      );
    } finally {
      delete (window as { matchMedia?: unknown }).matchMedia;
    }
  });

  it('says where a play starts before it starts', async () => {
    commands.favoritesList.mockResolvedValue(ok([saved('Sinnerman')]));
    await AppShellWrapper.mount('/library?view=favorites');

    await userEvent.click(await screen.findByRole('button', { name: 'Shuffle' }));

    await waitFor(() => expect(commands.collectionPlay).toHaveBeenCalled());
    expect(commands.playbackArmContext).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'favorites', to: '/library', search: { view: 'favorites' } }),
    );
    expect(commands.playbackArmContext.mock.invocationCallOrder[0]).toBeLessThan(
      commands.collectionPlay.mock.invocationCallOrder[0] ?? 0,
    );
    // Favorites is only the player's: Home's Your music doesn't list it.
    expect(commands.recentContextsAdd).not.toHaveBeenCalled();
  });
});

describe('reveal keys', () => {
  it('matches a row by its song, whatever the credit or decoration', () => {
    const row = songReveal('Get Lucky - Radio Edit', 'Daft Punk, Pharrell Williams');
    expect(matchesReveal(row, songReveal('Get Lucky', 'Daft Punk').split('\t'))).toBe(true);
    expect(matchesReveal(row, songReveal('Lose Yourself', 'Daft Punk').split('\t'))).toBe(false);
    expect(matchesReveal('episode:a', ['episode:a'])).toBe(true);
    expect(matchesReveal('episode:a', ['episode:b'])).toBe(false);
  });
});
