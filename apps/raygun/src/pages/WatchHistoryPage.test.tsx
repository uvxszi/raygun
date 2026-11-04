import type { ContinueWatching } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const entry = (episode: number, updatedAt: number, extra: Partial<ContinueWatching> = {}) => ({
  key: `stremio:series:tt1:1:${episode}`,
  title: 'Breaking Bad',
  subtitle: `S1 E${episode} · Episode ${episode}`,
  poster: null,
  positionMs: 60_000,
  durationMs: 600_000,
  updatedAt,
  watched: false,
  ...extra,
});

describe('Watch history', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('lists episodes by day with which one each is, and resumes one', async () => {
    const user = userEvent.setup();
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    commands.videoHistory.mockResolvedValue(
      ok([entry(2, Date.now()), entry(1, yesterday.getTime(), { watched: true })]),
    );
    await AppShellWrapper.mount('/watch/history');
    const page = await screen.findByTestId('watch-history-page');

    expect(await within(page).findByRole('heading', { name: 'Today' })).toBeInTheDocument();
    expect(within(page).getByRole('heading', { name: 'Yesterday' })).toBeInTheDocument();
    const second = within(page).getByRole('listitem', {
      name: 'Breaking Bad · S1 E2 · Episode 2',
    });
    expect(second).toHaveTextContent('S1 E2 · Episode 2');
    expect(
      within(page).getByRole('listitem', { name: 'Breaking Bad · S1 E1 · Episode 1' }),
    ).toHaveTextContent('Watched');

    await user.click(within(second).getByRole('button', { name: /^Breaking Bad/ }));
    expect(commands.videoResume).toHaveBeenCalledWith('stremio:series:tt1:1:2');
  });

  it('follows the player and asks before removing an entry', async () => {
    const user = userEvent.setup();
    commands.videoHistory.mockResolvedValue(ok([entry(1, Date.now())]));
    await AppShellWrapper.mount('/watch/history');
    const row = await screen.findByRole('listitem', { name: 'Breaking Bad · S1 E1 · Episode 1' });

    // Something was watched: the list is read again.
    commands.videoHistory.mockResolvedValue(ok([entry(2, Date.now()), entry(1, Date.now() - 5)]));
    emitEvent('watchChangedEvent', 'stremio:series:tt1:1:2');
    expect(
      await screen.findByRole('listitem', { name: 'Breaking Bad · S1 E2 · Episode 2' }),
    ).toBeInTheDocument();

    await user.click(
      within(row).getByRole('button', { name: 'Remove Breaking Bad · S1 E1 · Episode 1' }),
    );
    expect(commands.videoRemove).not.toHaveBeenCalled();
    await user.click(within(row).getByRole('button', { name: 'Remove' }));
    expect(commands.videoRemove).toHaveBeenCalledWith('stremio:series:tt1:1:1');
  });

  it('sends an empty history to Browse → Search for videos', async () => {
    await AppShellWrapper.mount('/watch/history');
    const page = await screen.findByTestId('watch-history-page');
    expect(await within(page).findByText('Nothing watched yet')).toBeInTheDocument();
    expect(within(page).getByRole('link', { name: 'Find in Browse' })).toHaveAttribute(
      'href',
      '/browse?tab=search&type=video',
    );
  });
});
