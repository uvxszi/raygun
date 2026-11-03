import { screen, within } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

describe('What Raygun can run', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('lists every system with what it brings, and says what other players became', async () => {
    await AppShellWrapper.mount('/extensions/systems');
    const page = await screen.findByTestId('extension-systems');
    const items = within(page).getAllByRole('listitem');
    expect(items.map((item) => item.getAttribute('aria-label'))).toEqual([
      'Raygun extensions',
      'Nuclear plugins',
      'Shosetsu sources',
      'Stremio addons',
      'Podcast and audiobook feeds',
      'Websites',
      'Imports',
      'Other players',
    ]);
    expect(within(page).getByRole('listitem', { name: 'Other players' })).toHaveTextContent(
      'QuietPlay and Musike have no add-ons to load',
    );
    expect(
      within(within(page).getByRole('listitem', { name: 'Stremio addons' })).getByRole('link', {
        name: 'Manage',
      }),
    ).toHaveAttribute('href', '/watch/addons');
  });
});
