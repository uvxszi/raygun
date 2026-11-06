import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

describe('Erase everything', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('erases only once ERASE is typed out, forgetting what the webview kept', async () => {
    const user = userEvent.setup();
    localStorage.setItem('raygun.sourceHealth', '{"x":1}');
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('erase-card');

    await user.click(within(card).getByRole('button', { name: /Erase everything/ }));
    const sheet = await screen.findByTestId('erase-sheet');
    expect(sheet).toHaveTextContent('Backups stay');
    const confirm = within(sheet).getByRole('button', { name: 'Erase and restart' });
    const field = within(sheet).getByRole('textbox', { name: 'Type ERASE to confirm' });
    expect(confirm).toBeDisabled();

    await user.type(field, 'erase');
    expect(confirm).toBeDisabled();
    await user.clear(field);
    await user.type(field, 'ERASE');
    expect(confirm).toBeEnabled();
    await user.click(confirm);

    expect(commands.dataEraseEverything).toHaveBeenCalledWith('ERASE');
    expect(await within(sheet).findByText('Raygun is starting again.')).toBeInTheDocument();
    expect(localStorage.getItem('raygun.sourceHealth')).toBeNull();
  });

  it('keeps everything when cancelled', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('erase-card');

    await user.click(within(card).getByRole('button', { name: /Erase everything/ }));
    const sheet = await screen.findByTestId('erase-sheet');
    await user.type(within(sheet).getByRole('textbox'), 'ERASE');
    await user.click(within(sheet).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByTestId('erase-sheet')).not.toBeInTheDocument();
    expect(commands.dataEraseEverything).not.toHaveBeenCalled();
  });
});
