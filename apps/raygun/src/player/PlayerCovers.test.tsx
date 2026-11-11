import { waitFor, within } from '@testing-library/react';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const pictureIn = (element: HTMLElement) => within(element).queryByTestId('artwork-image');

describe('the playing song’s cover', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.coverFallback.mockReset();
    commands.coverFallback.mockResolvedValue({ status: 'ok', data: null });
  });

  it('takes a cover given to the playing song at once, in the bar and the full player', async () => {
    await AppShellWrapper.mount();
    const song = makeItem('Sinnerman', 'Nina Simone');
    AppShellWrapper.playing([song], 0);
    await AppShellWrapper.miniPlayer.title;
    expect(pictureIn(AppShellWrapper.miniPlayer.element)).toBeNull();

    // Edit details picked a cover for it: Rust's queue carries it.
    const picked = 'http://127.0.0.1:9/cover/picked';
    AppShellWrapper.playing([{ ...song, artworkUrl: picked }], 0);
    await waitFor(() =>
      expect(pictureIn(AppShellWrapper.miniPlayer.element)).toHaveAttribute('src', picked),
    );
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;
    expect(within(sheet).getByTestId('player-artwork').querySelector('img')).toHaveAttribute(
      'src',
      picked,
    );
  });

  it('shows the cover found elsewhere in the full player, as the song’s row does', async () => {
    const found = 'http://127.0.0.1:9/cover/found';
    commands.coverFallback.mockResolvedValue({ status: 'ok', data: found });
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Be My Husband', 'Nina Simone')], 0);
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;
    await waitFor(() =>
      expect(within(sheet).getByTestId('player-artwork').querySelector('img')).toHaveAttribute(
        'src',
        found,
      ),
    );
    expect(commands.coverFallback).toHaveBeenCalledWith({
      kind: 'song',
      query: { title: 'Be My Husband', artist: 'Nina Simone', album: null, durationMs: 180_000 },
    });
  });
});
