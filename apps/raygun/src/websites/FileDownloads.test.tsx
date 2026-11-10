import type { WebsiteDownload } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const fail = (error: string) => ({ status: 'error' as const, error });

const file = (overrides: Partial<WebsiteDownload>): WebsiteDownload => ({
  id: 'f1',
  url: 'https://archive.org/download/book.pdf',
  name: 'book.pdf',
  mime: 'application/pdf',
  site: 'archive.org',
  state: 'downloading',
  doneBytes: 512 * 1024,
  totalBytes: 2 * 1024 * 1024,
  bytesPerSecond: 256 * 1024,
  resumable: true,
  folder: 'Download/Raygun',
  shown: null,
  error: null,
  createdAt: 1,
  ...overrides,
});

describe('Downloads → Files', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows a running download with its progress and cancels it', async () => {
    const user = userEvent.setup();
    commands.websiteDownloads.mockResolvedValue([file({})]);
    await AppShellWrapper.mount('/downloads?tab=files');
    const row = await screen.findByRole('listitem', { name: 'book.pdf' });
    expect(row).toHaveTextContent('archive.org · 512 KB of 2.0 MB · 256 KB/s');
    expect(within(row).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
    await user.click(within(row).getByRole('button', { name: 'Cancel book.pdf' }));
    expect(commands.websiteDownloadCancel).not.toHaveBeenCalled();
    const ask = await screen.findByRole('dialog', { name: 'Stop this download?' });
    await user.click(within(ask).getByRole('button', { name: 'Stop' }));
    expect(commands.websiteDownloadCancel).toHaveBeenCalledWith('f1');
  });

  it('follows the service: a finished file opens, shows its folder, and deletes after asking', async () => {
    const user = userEvent.setup();
    commands.websiteDownloads.mockResolvedValue([file({})]);
    await AppShellWrapper.mount('/downloads?tab=files');
    await screen.findByRole('listitem', { name: 'book.pdf' });

    commands.websiteDownloads.mockResolvedValue([
      file({ state: 'done', doneBytes: 2 * 1024 * 1024, bytesPerSecond: null }),
    ]);
    emitEvent('websiteDownloadsEvent', 'f1');
    const open = await screen.findByRole('button', { name: 'Open' });
    const row = screen.getByRole('listitem', { name: 'book.pdf' });
    expect(row).toHaveTextContent('2.0 MB · Download/Raygun');

    await user.click(open);
    expect(commands.websiteDownloadOpen).toHaveBeenCalledWith('f1');
    await user.click(within(row).getByRole('button', { name: 'Show in folder' }));
    expect(commands.websiteDownloadReveal).toHaveBeenCalledWith('f1');

    // Delete asks first; Cancel keeps the file.
    await user.click(within(row).getByRole('button', { name: 'Delete' }));
    const ask = within(row).getByRole('alertdialog', { name: 'Delete book.pdf from this device?' });
    await user.click(within(ask).getByRole('button', { name: 'Cancel' }));
    expect(commands.websiteDownloadRemove).not.toHaveBeenCalled();

    await user.click(within(row).getByRole('button', { name: 'Delete' }));
    commands.websiteDownloads.mockResolvedValue([]);
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Delete' }),
    );
    expect(commands.websiteDownloadRemove).toHaveBeenCalledWith('f1', true);
    expect(await screen.findByText('No files from websites yet')).toBeInTheDocument();
  });

  it('explains a failed download and resumes it', async () => {
    const user = userEvent.setup();
    commands.websiteDownloads.mockResolvedValue([
      file({ state: 'failed', error: 'The connection broke', bytesPerSecond: null }),
    ]);
    await AppShellWrapper.mount('/downloads?tab=files');
    const row = await screen.findByRole('listitem', { name: 'book.pdf' });
    expect(row).toHaveTextContent("Couldn't download from archive.org");
    expect(row).toHaveTextContent("Didn't download");
    await user.click(within(row).getByRole('button', { name: 'Resume' }));
    expect(commands.websiteDownloadRetry).toHaveBeenCalledWith('f1');

    await user.click(within(row).getByRole('button', { name: 'Remove book.pdf from the list' }));
    expect(commands.websiteDownloadRemove).toHaveBeenCalledWith('f1', false);
  });

  it('says what went wrong when a file no longer opens', async () => {
    const user = userEvent.setup();
    commands.websiteDownloads.mockResolvedValue([file({ state: 'done', bytesPerSecond: null })]);
    commands.websiteDownloadOpen.mockResolvedValue(fail("The file isn't there any more"));
    await AppShellWrapper.mount('/downloads?tab=files');
    await user.click(await screen.findByRole('button', { name: 'Open' }));
    expect(await screen.findByText("That didn't work")).toBeInTheDocument();
  });

  it('lists files on Downloads → All', async () => {
    commands.websiteDownloads.mockResolvedValue([file({})]);
    await AppShellWrapper.mount('/downloads');
    expect(await screen.findByRole('region', { name: 'Files' })).toBeInTheDocument();
    expect(screen.getByRole('listitem', { name: 'book.pdf' })).toBeInTheDocument();
  });

  it('shows an empty list plainly', async () => {
    await AppShellWrapper.mount('/downloads?tab=files');
    expect(await screen.findByText('No files from websites yet')).toBeInTheDocument();
  });
});
