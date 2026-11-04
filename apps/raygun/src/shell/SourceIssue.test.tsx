import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

describe('Source issues', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('says calmly which source had an issue, with the details on request', async () => {
    const user = userEvent.setup();
    commands.musicChart.mockResolvedValue({ status: 'error', error: 'Apple answered HTTP 503' });
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    const issue = await screen.findByTestId('source-issue');
    expect(issue).toHaveTextContent('Apple is having problems — try later');
    expect(within(issue).getByText('Apple answered HTTP 503')).not.toBeVisible();
    await user.click(within(issue).getByRole('button', { name: 'More info' }));
    expect(within(issue).getByText('Apple answered HTTP 503')).toBeVisible();
    expect(within(issue).getByRole('button', { name: 'Copy details' })).toBeInTheDocument();
  });

  it('shows the sentence Rust sends and keeps its details under More info', async () => {
    const user = userEvent.setup();
    commands.musicChart.mockResolvedValue({
      status: 'error',
      error:
        'Tunes wants a browser check, which Raygun can’t do\napi.tunes.example answered HTTP 403 with a browser check',
    });
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    const issue = await screen.findByTestId('source-issue');
    expect(
      within(issue).getByText('Tunes wants a browser check, which Raygun can’t do'),
    ).toBeVisible();
    const details = within(issue).getByText(
      'api.tunes.example answered HTTP 403 with a browser check',
    );
    expect(details).not.toBeVisible();
    await user.click(within(issue).getByRole('button', { name: 'More info' }));
    expect(details).toBeVisible();
  });

  it('shows a plain message as it is, with nothing more to tell', async () => {
    commands.musicChart.mockResolvedValue({
      status: 'error',
      error: 'Spotify has no such list; it may be private or removed',
    });
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    const issue = await screen.findByTestId('source-issue');
    expect(
      within(issue).getByText('Spotify has no such list; it may be private or removed'),
    ).toBeVisible();
    expect(within(issue).queryByRole('button', { name: 'More info' })).not.toBeInTheDocument();
  });
});
