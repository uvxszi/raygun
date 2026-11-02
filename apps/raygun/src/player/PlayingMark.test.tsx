import { screen, waitFor, within } from '@testing-library/react';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent, IDLE_STATE } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const song = (title: string, artist: string) => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: 180_000,
  artworkUrl: null,
  source: null,
});

describe('The playing mark', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('marks the playing song wherever it is listed, still while paused', async () => {
    commands.musicArtist.mockResolvedValue(
      ok({
        name: 'Raygun',
        popular: [song('Intro', 'Raygun'), song('Second', 'Raygun')],
        albums: [],
        tracks: [],
      }),
    );
    await AppShellWrapper.mount('/artist?name=Raygun');
    AppShellWrapper.playing([makeItem('Intro', 'Raygun'), makeItem('Second')], 0);

    const rows = await screen.findAllByTestId('online-track-row');
    const playing = await within(rows[0]!).findByTestId('now-playing-bars');
    expect(playing).toHaveAttribute('data-playing', 'true');
    expect(within(rows[1]!).queryByTestId('now-playing-bars')).not.toBeInTheDocument();

    emitEvent('playbackStateEvent', { ...IDLE_STATE, status: 'paused', currentItemId: 'id-Intro' });
    await waitFor(() =>
      expect(within(rows[0]!).getByTestId('now-playing-bars')).toHaveAttribute(
        'data-playing',
        'false',
      ),
    );
  });

  it('asks for what played last once the player is ready', async () => {
    await AppShellWrapper.mount();
    await waitFor(() => expect(commands.playbackRestore).toHaveBeenCalledTimes(1), {
      timeout: 3000,
    });
  });
});
