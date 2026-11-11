import type { PlaybackState } from '@raygun/bindings';
import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { bindingsMock, emitEvent, IDLE_STATE, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { PlayPauseButton } from './transport';
import { usePlaybackSync } from './usePlayback';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** The button with the shell's sync, so Rust's state events reach it. */
const SyncedButton: FC = () => {
  usePlaybackSync();
  return <PlayPauseButton />;
};

/** Lets the query cache tell the views what changed (it does so a moment later). */
const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 0)));

const songState = (state: Partial<PlaybackState>): PlaybackState => ({
  ...IDLE_STATE,
  currentItemId: 'song',
  positionMs: 60_000,
  durationMs: 180_000,
  ...state,
});

/** The button once the player's `state` is in: it reads `name` then. */
const showing = (state: Partial<PlaybackState>, name: 'Pause' | 'Play') => {
  commands.playbackGetState.mockResolvedValue({
    ...IDLE_STATE,
    currentItemId: 'song',
    durationMs: 180_000,
    ...state,
  });
  renderWithProviders(<PlayPauseButton />);
  return screen.findByRole('button', { name });
};

describe('Play and pause', () => {
  beforeEach(() => {
    seedBindings();
  });

  it('turns a ring around Pause while a stream is found or buffers', async () => {
    const button = await showing({ status: 'loading', waiting: true }, 'Pause');
    expect(button).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByTestId('play-waiting')).toBeInTheDocument();
  });

  it('shows a downloaded song starting as Pause, without a ring', async () => {
    const user = userEvent.setup();
    // The engine was given the song's file: it starts at once, nothing to wait for.
    const button = await showing({ status: 'loading', waiting: false }, 'Pause');
    expect(button).not.toHaveAttribute('aria-busy');
    expect(screen.queryByTestId('play-waiting')).not.toBeInTheDocument();

    await user.click(button);
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'toggle' });
  });

  it('shows no ring while a file on the device buffers', async () => {
    await showing({ status: 'buffering', waiting: false }, 'Pause');
    expect(screen.queryByTestId('play-waiting')).not.toBeInTheDocument();
  });

  it('waits while another device that says nothing about it loads', async () => {
    // An older Raygun on the other end of Connect doesn't say whether it waits.
    await showing({ status: 'buffering' }, 'Pause');
    expect(screen.getByTestId('play-waiting')).toBeInTheDocument();
  });

  it('shows Play without a ring while paused', async () => {
    await showing({ status: 'paused', waiting: false }, 'Play');
    expect(screen.queryByTestId('play-waiting')).not.toBeInTheDocument();
  });

  it('flips on the tap, before Rust answers, and a report from before stays off screen', async () => {
    const user = userEvent.setup();
    // Rust takes its time to answer.
    commands.playbackControl.mockReturnValue(new Promise(() => undefined));
    commands.playbackGetState.mockResolvedValue(songState({ status: 'playing', waiting: false }));
    renderWithProviders(<SyncedButton />);

    await user.click(await screen.findByRole('button', { name: 'Pause' }));
    await settle();
    expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument();
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'toggle' });

    // A position report sent before the pause reached Rust doesn't flip it back.
    emitEvent('playbackStateEvent', songState({ status: 'playing', positionMs: 61_000 }));
    await settle();
    expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument();
    // Rust's answer agrees.
    emitEvent('playbackStateEvent', songState({ status: 'paused', positionMs: 61_200 }));
    await settle();
    expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument();

    // Played again: Pause at once, with no ring for a file that only resumes.
    await user.click(screen.getByRole('button', { name: 'Play' }));
    await settle();
    const pause = screen.getByRole('button', { name: 'Pause' });
    expect(pause).not.toHaveAttribute('aria-busy');
    expect(screen.queryByTestId('play-waiting')).not.toBeInTheDocument();
    emitEvent('playbackStateEvent', songState({ status: 'playing', waiting: false }));
    await settle();
    expect(screen.getByRole('button', { name: 'Pause' })).not.toHaveAttribute('aria-busy');
  });

  it('shows the ring once Rust says a stream really waits', async () => {
    const user = userEvent.setup();
    commands.playbackGetState.mockResolvedValue(songState({ status: 'paused', waiting: false }));
    renderWithProviders(<SyncedButton />);

    await user.click(await screen.findByRole('button', { name: 'Play' }));
    await settle();
    expect(screen.getByRole('button', { name: 'Pause' })).not.toHaveAttribute('aria-busy');
    // Reopened paused, its stream was never loaded: Rust says it waits for it.
    emitEvent('playbackStateEvent', songState({ status: 'loading', waiting: true }));
    expect(await screen.findByTestId('play-waiting')).toBeInTheDocument();
  });
});
