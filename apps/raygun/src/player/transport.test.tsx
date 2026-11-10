import type { PlaybackState } from '@raygun/bindings';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock, IDLE_STATE, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { PlayPauseButton } from './transport';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

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
});
