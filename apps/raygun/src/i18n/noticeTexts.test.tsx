import { waitFor } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

describe("Rust's notification texts", () => {
  it("are sent in the app's language once it's known", async () => {
    AppShellWrapper.seedCore();
    await AppShellWrapper.mount('/');
    await waitFor(() =>
      expect(commands.appTextsSet).toHaveBeenCalledWith(
        'en',
        expect.objectContaining({
          'news.updateOut': 'Raygun {{version}} is out',
          'news.newChapters_other': '{{count}} new chapters',
          'actions.cancelAll': 'Cancel all',
          'downloads.title': 'Downloads',
        }),
      ),
    );
  });
});
