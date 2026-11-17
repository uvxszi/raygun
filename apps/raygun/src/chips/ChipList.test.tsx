import type { ChipInfo } from '@raygun/bindings';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '../test/renderWithProviders';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { ChipList } from './ChipList';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const chip = (id: ChipInfo['id'], over: Partial<ChipInfo> = {}): ChipInfo => ({
  id,
  name: id,
  description: `${id} does things`,
  kind: 'feature',
  version: '1.0.0',
  size: 2048,
  installed: false,
  outdated: false,
  on: true,
  progress: { kind: 'idle' },
  ...over,
});

describe('Smart Chips list', () => {
  it('installs a chip, shows its download and then its switch', async () => {
    const user = userEvent.setup();
    commands.chipsList.mockResolvedValue([chip('smartShuffle'), chip('smoothMotion')]);
    commands.chipsInstall.mockResolvedValue({
      status: 'ok',
      data: [chip('smartShuffle', { installed: true }), chip('smoothMotion')],
    });
    renderWithProviders(<ChipList />);

    const row = await screen.findByTestId('chip-smartShuffle');
    expect(within(row).getByText('Smart Shuffle')).toBeInTheDocument();
    expect(within(row).getByText('Size: 2.0 KB')).toBeInTheDocument();
    await user.click(within(row).getByRole('button', { name: 'Install' }));
    expect(commands.chipsInstall).toHaveBeenCalledWith('smartShuffle');

    // Progress arrives as events while it downloads.
    act(() =>
      emitEvent('chipsEvent', [
        chip('smartShuffle', { progress: { kind: 'downloading', received: 1024, total: 2048 } }),
        chip('smoothMotion'),
      ]),
    );
    const bar = await within(screen.getByTestId('chip-smartShuffle')).findByRole('progressbar');
    expect(bar).toHaveAttribute('aria-valuenow', '50');
    await user.click(
      within(screen.getByTestId('chip-smartShuffle')).getByRole('button', { name: 'Cancel' }),
    );
    expect(commands.chipsCancel).toHaveBeenCalledWith('smartShuffle');

    act(() =>
      emitEvent('chipsEvent', [chip('smartShuffle', { installed: true }), chip('smoothMotion')]),
    );
    expect(
      await within(screen.getByTestId('chip-smartShuffle')).findByRole('switch', {
        name: 'Smart Shuffle on',
      }),
    ).toHaveAttribute('aria-checked', 'true');
  });

  it('switches an installed chip off and removes it after asking', async () => {
    const user = userEvent.setup();
    commands.chipsList.mockResolvedValue([chip('smoothMotion', { installed: true })]);
    commands.chipsSetEnabled.mockResolvedValue({
      status: 'ok',
      data: [chip('smoothMotion', { installed: true, on: false })],
    });
    commands.chipsRemove.mockResolvedValue({ status: 'ok', data: [chip('smoothMotion')] });
    renderWithProviders(<ChipList />);

    const row = await screen.findByTestId('chip-smoothMotion');
    await user.click(within(row).getByRole('switch', { name: 'Smooth Motion on' }));
    expect(commands.chipsSetEnabled).toHaveBeenCalledWith('smoothMotion', false);
    expect(await within(row).findByRole('switch')).toHaveAttribute('aria-checked', 'false');

    await user.click(within(row).getByRole('button', { name: 'Remove' }));
    expect(commands.chipsRemove).not.toHaveBeenCalled();
    await user.click(within(row).getByRole('button', { name: 'Remove Smooth Motion' }));
    expect(commands.chipsRemove).toHaveBeenCalledWith('smoothMotion');
    expect(await within(row).findByRole('button', { name: 'Install' })).toBeInTheDocument();
  });

  it('says why an install failed and offers updates', async () => {
    commands.chipsList.mockResolvedValue([
      chip('smartSubtitles', { progress: { kind: 'failed', message: 'the package is damaged' } }),
      chip('smoothMotion', { installed: true, outdated: true }),
    ]);
    renderWithProviders(<ChipList />);
    expect(await screen.findByRole('alert')).toHaveTextContent('the package is damaged');
    expect(
      within(screen.getByTestId('chip-smoothMotion')).getByRole('button', { name: 'Update' }),
    ).toBeInTheDocument();
  });
});
