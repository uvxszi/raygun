import type { LocalFolder } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const MEDIA: LocalFolder = {
  id: 'f1',
  name: 'Media',
  location: 'C:/Users/me/Media',
  counts: { songs: 12, comics: 3, books: 1, videos: 0 },
  scannedAt: 1,
  scanning: false,
};

describe('Local Files (M46.27)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('adds a folder, shows what it holds, and removes it', async () => {
    const user = userEvent.setup();
    commands.localFolderAdd.mockResolvedValue(ok([{ ...MEDIA, scanning: true }]));
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    expect(await within(page).findByText('No folders yet')).toBeInTheDocument();

    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const folder = await within(page).findByRole('listitem', { name: 'Media' });
    expect(folder).toHaveTextContent('Scanning…');

    // The scan finished.
    commands.localFolders.mockResolvedValue(ok([MEDIA]));
    emitEvent('localFilesEvent', 'f1');
    await vi.waitFor(() => expect(folder).toHaveTextContent('12 songs · 3 comics · 1 book'));

    await user.click(within(folder).getByRole('button', { name: 'Remove Media' }));
    expect(commands.localFolderRemove).toHaveBeenCalledWith('f1');
  });

  it('is a green block in the Library, always there (M46.29)', async () => {
    commands.localFolders.mockResolvedValue(ok([MEDIA]));
    await AppShellWrapper.mount('/library?media=music');
    const overview = await screen.findByTestId('music-overview');
    expect(await within(overview).findByRole('link', { name: /Local files/ })).toHaveAttribute(
      'href',
      expect.stringContaining('/local-files'),
    );
    expect(within(overview).getByRole('link', { name: /Downloaded/ })).toBeInTheDocument();
    expect(await within(overview).findByText('16 items')).toBeInTheDocument();
  });
});
