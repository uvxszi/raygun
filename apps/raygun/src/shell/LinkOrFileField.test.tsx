import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import { bindingsMock } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { LinkOrFileField } from './LinkOrFileField';
import type { LinkOrFilePick } from './LinkOrFileField';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const Field: FC<{ pick?: LinkOrFilePick; onPicked?: (path: string) => void }> = ({
  pick,
  onPicked,
}) => {
  const [value, setValue] = useState('');
  return (
    <LinkOrFileField
      pick={pick}
      value={value}
      onChange={setValue}
      onPicked={onPicked}
      aria-label="Address"
    />
  );
};

describe('LinkOrFileField', () => {
  it('takes a typed link', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Field />);
    await user.type(screen.getByRole('textbox', { name: 'Address' }), 'https://example.org/x');
    expect(screen.getByRole('textbox', { name: 'Address' })).toHaveValue('https://example.org/x');
  });

  it('fills itself with a picked file, so the path needs no typing', async () => {
    const user = userEvent.setup();
    const onPicked = vi.fn();
    commands.filesPick.mockResolvedValue(ok({ path: '/music/song.flac', name: 'song.flac' }));
    renderWithProviders(<Field onPicked={onPicked} />);
    // Only files: the button picks one at once.
    await user.click(screen.getByRole('button', { name: 'Choose file' }));
    expect(commands.filesPick).toHaveBeenCalledWith('file');
    expect(screen.getByRole('textbox', { name: 'Address' })).toHaveValue('/music/song.flac');
    expect(onPicked).toHaveBeenCalledWith('/music/song.flac');
  });

  it('asks whether a file or a folder, and keeps the text when nothing is picked', async () => {
    const user = userEvent.setup();
    const onPicked = vi.fn();
    commands.filesPick.mockResolvedValue(ok(null));
    renderWithProviders(<Field pick="both" onPicked={onPicked} />);
    const field = screen.getByRole('textbox', { name: 'Address' });
    await user.type(field, 'typed');
    const button = screen.getByRole('button', { name: 'Choose a file or folder' });
    await user.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    const menu = screen.getByRole('menu', { name: 'Choose a file or folder' });
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent),
    ).toEqual(['Choose file', 'Choose folder']);
    await user.click(within(menu).getByRole('menuitem', { name: 'Choose folder' }));
    expect(commands.filesPick).toHaveBeenCalledWith('folder');
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(field).toHaveValue('typed');
    expect(onPicked).not.toHaveBeenCalled();
  });

  it('says why a pick failed', async () => {
    const user = userEvent.setup();
    commands.filesPick.mockResolvedValue({ status: 'error', error: 'That file is too large' });
    renderWithProviders(<Field pick="folder" />);
    await user.click(screen.getByRole('button', { name: 'Choose folder' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('That file is too large');
    // Typing again clears it.
    await user.type(screen.getByRole('textbox', { name: 'Address' }), 'x');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
