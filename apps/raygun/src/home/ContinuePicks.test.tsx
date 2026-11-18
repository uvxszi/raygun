import type { ListeningItem, SavedTrack } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const reading = [
  {
    medium: 'manga',
    entry: libraryEntry({ titleId: 't1', title: 'Frieren', lastReadAt: 2, sourceId: 's' }),
  },
  {
    medium: 'novel',
    entry: libraryEntry({ titleId: 't2', title: 'Dracula', lastReadAt: 1, sourceId: 's' }),
  },
];

const episode: ListeningItem = {
  kind: 'podcast',
  podcastId: 'p1',
  episodeId: 'e7',
  title: 'Episode Seven',
  show: 'The Example Show',
  author: null,
  artworkUrl: null,
  positionMs: 1_000,
  durationMs: 60_000,
  chapter: null,
  chapters: null,
  listenedAt: 1,
};

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

describe("Home's Continue rows: picking and the bar", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('takes picked titles out of Continue reading, keeping their places', async () => {
    const user = userEvent.setup();
    commands.continueReading.mockResolvedValue(ok(reading));
    await AppShellWrapper.mount('/');
    const row = await screen.findByTestId('continue-reading');
    expect(within(row).queryByTestId('continue-pick-bar')).not.toBeInTheDocument();

    await user.click(within(row).getByRole('checkbox', { name: 'Pick Frieren' }));
    const bar = await within(row).findByRole('toolbar', { name: 'Continue reading: actions' });
    expect(within(bar).getByText('1 selected')).toBeInTheDocument();
    await user.click(within(bar).getByRole('button', { name: 'Remove from Continue' }));
    await waitFor(() => expect(commands.continueHide).toHaveBeenCalledWith(['reading:t1']));
    await waitFor(() =>
      expect(within(row).queryByTestId('continue-pick-bar')).not.toBeInTheDocument(),
    );
  });

  it('marks picked comics and books read, each through its own medium', async () => {
    const user = userEvent.setup();
    commands.continueReading.mockResolvedValue(ok(reading));
    commands.libraryMarkRead.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/');
    const row = await screen.findByTestId('continue-reading');
    await user.click(within(row).getByRole('checkbox', { name: 'Pick Frieren' }));
    const bar = await within(row).findByRole('toolbar');
    await user.click(within(bar).getByRole('button', { name: 'Select all' }));
    await user.click(within(bar).getByRole('button', { name: 'Mark as read' }));
    await waitFor(() =>
      expect(commands.libraryMarkRead).toHaveBeenCalledWith('manga', ['t1'], true),
    );
    expect(commands.libraryMarkRead).toHaveBeenCalledWith('novel', ['t2'], true);
  });

  it('asks before taking picked titles out of the library', async () => {
    const user = userEvent.setup();
    commands.continueReading.mockResolvedValue(ok(reading));
    commands.libraryRemove.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/');
    const row = await screen.findByTestId('continue-reading');
    await user.click(within(row).getByRole('checkbox', { name: 'Pick Dracula' }));
    await user.click(await within(row).findByRole('button', { name: 'Remove from library' }));
    expect(commands.libraryRemove).not.toHaveBeenCalled();
    const ask = within(row).getByRole('alertdialog');
    await user.click(within(ask).getByRole('button', { name: 'Remove from library' }));
    await waitFor(() =>
      expect(commands.libraryRemove).toHaveBeenCalledWith('novel', ['t2'], true, false),
    );
  });

  it('marks picked episodes played and takes others out of Continue listening', async () => {
    const user = userEvent.setup();
    commands.podcastContinueListening.mockResolvedValue(ok([episode]));
    commands.podcastSetPlayedPicks.mockResolvedValue(ok(1));
    await AppShellWrapper.mount('/');
    const row = await screen.findByTestId('continue-listening');
    await user.click(
      within(row).getByRole('checkbox', { name: 'Pick The Example Show: Episode Seven' }),
    );
    await user.click(await within(row).findByRole('button', { name: 'Mark as played' }));
    await waitFor(() =>
      expect(commands.podcastSetPlayedPicks).toHaveBeenCalledWith(
        { shows: [], episodes: ['e7'] },
        true,
      ),
    );

    await user.click(
      within(row).getByRole('checkbox', { name: 'Pick The Example Show: Episode Seven' }),
    );
    await user.click(await within(row).findByRole('button', { name: 'Remove from Continue' }));
    await waitFor(() => expect(commands.continueHide).toHaveBeenCalledWith(['listening:e7']));
  });

  it('takes picked songs out of Recently played', async () => {
    const user = userEvent.setup();
    commands.historyRecent.mockResolvedValue(
      ok([
        { track: saved('Sinnerman'), playedAt: 2 },
        { track: saved('Feeling Good'), playedAt: 1 },
      ]),
    );
    await AppShellWrapper.mount('/');
    const recent = await screen.findByTestId('recently-played');
    await user.click(await within(recent).findByRole('checkbox', { name: 'Pick Feeling Good' }));
    await user.click(
      await within(recent).findByRole('button', { name: 'Remove from Recently played' }),
    );
    await waitFor(() =>
      expect(commands.historyRemove).toHaveBeenCalledWith(['nina simone|feeling good']),
    );
    expect(commands.collectionPlay).not.toHaveBeenCalled();
  });
});
