import type { OnlineTrack } from '@raygun/bindings';
import { screen, waitFor } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song: OnlineTrack = {
  externalId: null,
  title: 'Monkeys Spinning Monkeys',
  artist: 'Kevin MacLeod',
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
  explicit: false,
};

describe('Covers found for songs shown without one', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.musicCharts.mockResolvedValue(
      ok({
        charts: [
          {
            provider: {
              id: 'dev.example.charts/charts',
              extensionId: 'dev.example.charts',
              kind: 'music.charts',
              name: 'Charts',
            },
            id: 'top',
            name: 'Top songs',
            description: null,
            artworkUrl: null,
            tag: null,
            group: 'top',
          },
        ],
        errors: [],
      }),
    );
    commands.musicChart.mockResolvedValue(ok([song]));
  });

  it("asks for a chart song's cover, and asks again once sources change", async () => {
    commands.coverFallback.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/');
    const card = await screen.findByTestId('chart-song');
    const query = {
      kind: 'song',
      query: {
        title: 'Monkeys Spinning Monkeys',
        artist: 'Kevin MacLeod',
        album: null,
        durationMs: null,
      },
    };
    await waitFor(() => expect(commands.coverFallback).toHaveBeenCalledWith(query));
    expect(card).toBeInTheDocument();

    // A source installed later has one: the miss is asked again.
    commands.coverFallback.mockResolvedValue(ok('http://127.0.0.1:1/cover/monkeys'));
    emitEvent('coverMissesForgottenEvent', 1);
    await waitFor(() => expect(commands.coverFallback).toHaveBeenCalledTimes(2));
  });
});
