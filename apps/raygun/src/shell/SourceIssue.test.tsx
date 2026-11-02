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
    expect(issue).toHaveTextContent('Issue with source');
    expect(within(issue).getByText('Apple answered HTTP 503')).not.toBeVisible();
    await user.click(within(issue).getByRole('button', { name: 'More info' }));
    expect(within(issue).getByText('Apple answered HTTP 503')).toBeVisible();
    expect(within(issue).getByRole('button', { name: 'Copy details' })).toBeInTheDocument();
  });

  it('adds the test sources from Settings → Data', async () => {
    const user = userEvent.setup();
    commands.devAddTestSources.mockResolvedValue(
      ok({
        added: ['https://a.example/index.json', 'https://b.example/manifest.json'],
        already: [],
        failed: [{ url: 'https://c.example/manifest.json', error: 'HTTP 404' }],
      }),
    );
    await AppShellWrapper.mount('/settings/data');

    const card = await screen.findByTestId('developer-card');
    await user.click(within(card).getByRole('button', { name: 'Add test sources' }));
    expect(commands.devAddTestSources).toHaveBeenCalled();
    expect(await within(card).findByTestId('developer-report')).toHaveTextContent(
      '2 added, 0 already there, 1 couldn’t be added.',
    );
    expect(within(card).getByTestId('source-issue')).toHaveTextContent(
      'Issue with https://c.example/manifest.json',
    );
  });
});
