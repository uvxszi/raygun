import type { SettingChange, Settings } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';

import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';
import { AppShellWrapper } from './AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

describe('Panels', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withSettings(DEFAULT_SETTINGS);
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: true,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('lists playlists in the rail, which resizes down to covers only', async () => {
    await AppShellWrapper.mount('/');
    const rail = await screen.findByTestId('side-rail');
    expect(await within(rail).findByText('Road trip')).toBeInTheDocument();

    const handle = screen.getByRole('separator', { name: 'Drag to resize the side rail' });
    fireEvent.keyDown(handle, { key: 'ArrowRight' });
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
        key: 'panels',
        value: expect.objectContaining({ railWidth: 240 }),
      }),
    );
    fireEvent.keyDown(handle, { key: 'Home' });
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
        key: 'panels',
        value: expect.objectContaining({ railWidth: 72 }),
      }),
    );
    // Narrow: the playlist is its cover, named for screen readers.
    await vi.waitFor(() => expect(within(rail).queryByText('Road trip')).not.toBeInTheDocument());
    expect(within(rail).getByRole('link', { name: 'Road trip' })).toBeInTheDocument();
  });
});
