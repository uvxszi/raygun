import { screen, within } from '@testing-library/react';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const episode = (number: number, positionMs: number) => ({
  key: `stremio:series:tt1:1:${number}`,
  title: 'The Show',
  subtitle: `S1 E${number}`,
  poster: null,
  positionMs,
  durationMs: 1_400_000,
  updatedAt: number,
  watched: false,
});

describe('live invalidation', () => {
  it('moves Home on to the episode the player saved', async () => {
    commands.videoContinueWatching.mockResolvedValue(ok([episode(1, 4_000)]));
    await AppShellWrapper.mount('/');
    const card = await screen.findByRole('listitem', { name: 'The Show' });
    expect(within(card).getByText('S1 E1')).toBeInTheDocument();

    // The player closed on episode 7.
    commands.videoContinueWatching.mockResolvedValue(ok([episode(7, 900_000)]));
    emitEvent('watchChangedEvent', 'stremio:series:tt1:1:7');
    expect(await screen.findByText('S1 E7')).toBeInTheDocument();
    expect(screen.queryByText('S1 E1')).not.toBeInTheDocument();
  });

  it('asks every shown answer again after a sync', async () => {
    commands.videoContinueWatching.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(commands.videoContinueWatching).toHaveBeenCalled());
    const asked = commands.videoContinueWatching.mock.calls.length;
    emitEvent('syncChangedEvent', 3);
    await vi.waitFor(() =>
      expect(commands.videoContinueWatching.mock.calls.length).toBeGreaterThan(asked),
    );
  });
});
