import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import '../i18n';
import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { IDLE_STATE, bindingsMock } from '../test/bindingsMock';
import { MiniplayerApp } from './MiniplayerApp';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const renderMiniplayer = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MiniplayerApp />
    </QueryClientProvider>,
  );

describe('The miniplayer window', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows the song and, over the artwork, the controls', async () => {
    const user = userEvent.setup();
    const items = [makeItem('Murder Most Foul', 'Bob Dylan'), makeItem('Key West')];
    commands.playbackGetQueue.mockResolvedValue({ items, order: [0, 1], context: null });
    commands.playbackGetState.mockResolvedValue({
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: items[0]?.id ?? null,
      positionMs: 710_000,
      durationMs: 1_014_000,
    });
    renderMiniplayer();
    const window = await screen.findByTestId('miniplayer');
    expect(await within(window).findByText('Bob Dylan')).toBeInTheDocument();
    expect(within(window).getByText('16:54')).toBeInTheDocument();

    await user.click(within(window).getByRole('button', { name: 'Pause' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'toggle' });
    await user.click(within(window).getByRole('button', { name: 'Next' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'next' });
    await user.click(within(window).getByRole('button', { name: 'Mute' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'setVolume', volume: 0 });
    await user.click(within(window).getByRole('button', { name: 'Back to Raygun' }));
    expect(commands.miniplayerClose).toHaveBeenCalledWith(true);
    await user.click(within(window).getByRole('button', { name: 'Close the miniplayer' }));
    expect(commands.miniplayerClose).toHaveBeenCalledWith(false);
  });

  it('shows a saved credit without the quotes a source left around it', async () => {
    const items = [makeItem('Chop Suey!', '"System of a Down"')];
    commands.playbackGetQueue.mockResolvedValue({ items, order: [0], context: null });
    commands.playbackGetState.mockResolvedValue({
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: items[0]?.id ?? null,
    });
    renderMiniplayer();
    const window = await screen.findByTestId('miniplayer');
    expect(await within(window).findByText('System of a Down')).toBeInTheDocument();
    expect(within(window).queryByText('"System of a Down"')).not.toBeInTheDocument();
  });

  it('says when nothing plays', async () => {
    renderMiniplayer();
    expect(await screen.findByText('Nothing playing')).toBeInTheDocument();
  });
});
