import { screen, within } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const TWITCH = {
  id: 'dev.raygun.twitch/twitch',
  name: 'Twitch',
  extensionId: 'dev.raygun.twitch',
  enabled: true,
  searchPill: 'Twitch',
};

describe('Live streams in a video source', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.videoSources.mockResolvedValue(ok([TWITCH]));
  });

  it('marks streams on the air with their viewers and what they stream', async () => {
    commands.videoBrowse.mockResolvedValue(
      ok({
        items: [
          {
            sourceId: TWITCH.id,
            id: 'afrochess',
            title: 'Blitz with viewers',
            coverUrl: null,
            inLibrary: false,
            subtitle: 'AfroChess · Chess',
            live: { viewers: 12_345, startedAt: 1_794_420_000_000 },
          },
          {
            sourceId: TWITCH.id,
            id: 'quietcorner',
            title: 'Just started',
            coverUrl: null,
            inLibrary: false,
            live: { viewers: null, startedAt: null },
          },
          {
            sourceId: TWITCH.id,
            id: 'offline',
            title: 'An offline channel',
            coverUrl: null,
            inLibrary: false,
          },
        ],
        hasNextPage: false,
      }),
    );
    await AppShellWrapper.mount(`/watch/source?id=${encodeURIComponent(TWITCH.id)}`);
    const page = await screen.findByTestId('manga-source-page');

    const chess = await within(page).findByRole('link', { name: 'Blitz with viewers' });
    expect(within(chess).getByRole('img', { name: 'Live, 12.3K watching' })).toBeVisible();
    expect(within(chess).getByText('AfroChess · Chess')).toBeVisible();

    const quiet = within(page).getByRole('link', { name: 'Just started' });
    expect(within(quiet).getByRole('img', { name: 'Live now' })).toBeVisible();

    const offline = within(page).getByRole('link', { name: 'An offline channel' });
    expect(within(offline).queryByTestId('live-mark')).not.toBeInTheDocument();
  });
});
