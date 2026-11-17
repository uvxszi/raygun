import type { PlaybackState, QueueSnapshot } from '@raygun/bindings';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { useContextMenuStore } from '../shell/contextMenuStore';
import { renderWithProviders } from '../test/renderWithProviders';
import { bindingsMock, IDLE_STATE } from '../test/bindingsMock';
import { nextShuffle, playsFromPlaylist, shuffleModeOf } from './smartShuffle';
import { SuggestedButton } from './SuggestedMark';
import { ShuffleButton } from './transport';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const smartChip = (on: boolean) => ({
  id: 'smartShuffle' as const,
  name: 'Smart Shuffle',
  description: '',
  kind: 'feature',
  version: '1.0.0',
  size: 1,
  installed: true,
  outdated: false,
  on,
  progress: { kind: 'idle' as const },
});

describe('Smart Shuffle', () => {
  it('cycles off, shuffle, Smart Shuffle while its chip is on', () => {
    expect(nextShuffle('off', true)).toEqual({ type: 'setShuffle', enabled: true });
    expect(nextShuffle('shuffle', true)).toEqual({ type: 'setSmartShuffle', enabled: true });
    expect(nextShuffle('smart', true)).toEqual({ type: 'setShuffle', enabled: false });
    // Without the chip, two states.
    expect(nextShuffle('shuffle', false)).toEqual({ type: 'setShuffle', enabled: false });
    const state = (shuffle: boolean, smartShuffle: boolean): PlaybackState => ({
      ...IDLE_STATE,
      shuffle,
      smartShuffle,
    });
    expect(shuffleModeOf(state(false, false))).toBe('off');
    expect(shuffleModeOf(state(true, false))).toBe('shuffle');
    expect(shuffleModeOf(state(true, true))).toBe('smart');
    expect(shuffleModeOf(undefined)).toBe('off');
  });

  it('the shuffle button asks for Smart Shuffle as its third state', async () => {
    const user = userEvent.setup();
    commands.chipsList.mockResolvedValue([smartChip(true)]);
    commands.playbackGetState.mockResolvedValue({ ...IDLE_STATE, shuffle: true });
    renderWithProviders(<ShuffleButton />);
    const button = await screen.findByRole('button', { name: 'Shuffle', pressed: true });
    await waitFor(() => expect(commands.chipsList).toHaveBeenCalled());
    await user.click(button);
    await waitFor(() =>
      expect(commands.playbackControl).toHaveBeenLastCalledWith({
        type: 'setSmartShuffle',
        enabled: true,
      }),
    );
  });

  it('shows a sparkle while Smart Shuffle is on, and the next press turns shuffle off', async () => {
    const user = userEvent.setup();
    commands.chipsList.mockResolvedValue([smartChip(true)]);
    commands.playbackGetState.mockResolvedValue({
      ...IDLE_STATE,
      shuffle: true,
      smartShuffle: true,
    });
    renderWithProviders(<ShuffleButton />);
    await user.click(await screen.findByRole('button', { name: 'Smart Shuffle' }));
    expect(screen.getByTestId('smart-shuffle-mark')).toBeInTheDocument();
    expect(commands.playbackControl).toHaveBeenLastCalledWith({
      type: 'setShuffle',
      enabled: false,
    });
  });

  it("marks a suggested song in the player and offers this playlist's actions", async () => {
    const user = userEvent.setup();
    const queue: QueueSnapshot = {
      items: [],
      order: [],
      context: {
        kind: 'playlist',
        name: 'Mine',
        subtitle: null,
        artworkUrl: null,
        to: '/library/playlist',
        search: { id: 'p1' },
        playedAt: 0,
      },
      suggested: ['s1'],
    };
    expect(playsFromPlaylist(queue.context ?? null)).toBe(true);
    commands.playbackGetQueue.mockResolvedValue(queue);
    commands.smartShuffleNotForMe.mockResolvedValue({ status: 'ok', data: null });
    renderWithProviders(
      <>
        <SuggestedButton itemId="s1" />
        <SuggestedButton itemId="own" />
      </>,
    );
    const sparkle = await screen.findByRole('button', { name: 'Suggested by Smart Shuffle' });
    expect(screen.getAllByTestId('suggested-button')).toHaveLength(1);
    await user.click(sparkle);
    // The menu's host lives in the shell; what it would show is in its store.
    const items = useContextMenuStore.getState().open?.items ?? [];
    const actions = items.flatMap((item) =>
      item.kind === undefined || item.kind === 'item' ? [item] : [],
    );
    expect(actions.map((item) => item.label)).toEqual(['Add to this playlist', 'Not for me']);
    actions[1]?.onSelect();
    expect(commands.smartShuffleNotForMe).toHaveBeenCalledWith('s1');
  });
});
