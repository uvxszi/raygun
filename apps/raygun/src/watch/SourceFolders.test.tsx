import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const PLUGIN = {
  id: 'kodi.plugin.video.example/video',
  name: 'Example films',
  extensionId: 'kodi.plugin.video.example',
  enabled: true,
};

describe("A source's folders (Kodi add-ons)", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.videoSources.mockResolvedValue(ok([PLUGIN]));
    commands.catalogFilters.mockResolvedValue(ok([]));
    commands.videoBrowse.mockResolvedValue(
      ok({
        items: [
          {
            sourceId: PLUGIN.id,
            id: '/?mode=films',
            title: 'Silent films',
            coverUrl: null,
            inLibrary: false,
            folder: true,
          },
          {
            sourceId: PLUGIN.id,
            id: '/?play=7',
            title: 'Nosferatu',
            coverUrl: null,
            inLibrary: false,
          },
        ],
        hasNextPage: false,
      }),
    );
    commands.catalogSearch.mockResolvedValue(
      ok({
        items: [
          {
            sourceId: PLUGIN.id,
            id: '/?play=8',
            title: 'The General',
            coverUrl: null,
            inLibrary: false,
          },
        ],
        hasNextPage: false,
      }),
    );
  });

  it('opens a folder as a list of its own, and a video as a title', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount(`/watch/source?id=${encodeURIComponent(PLUGIN.id)}`);
    const page = await screen.findByTestId('manga-source-page');

    expect(within(page).getByRole('link', { name: 'Nosferatu' })).toHaveAttribute(
      'href',
      expect.stringContaining('/watch/video'),
    );
    await user.click(await within(page).findByRole('link', { name: 'Silent films' }));

    const folder = await screen.findByTestId('manga-source-page');
    expect(await within(folder).findByRole('link', { name: 'The General' })).toBeVisible();
    expect(within(folder).getByRole('heading', { name: 'Silent films' })).toBeVisible();
    expect(commands.catalogSearch).toHaveBeenCalledWith('video', PLUGIN.id, '', 1, {
      section: '/?mode=films',
    });
    expect(within(folder).queryByRole('navigation', { name: 'Lists' })).not.toBeInTheDocument();
  });
});
