import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const QUEUE = [makeItem('Intro', 'Raygun'), makeItem('Second'), makeItem('Third')];

const setWide = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: true,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

describe('Playing like a power user', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('plays from the keyboard, and lists the shortcuts', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const page = screen.getByTestId('page-content');

    fireEvent.keyDown(page, { key: ' ' });
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'toggle' });
    fireEvent.keyDown(page, { key: 'ArrowRight', ctrlKey: true });
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'next' });
    fireEvent.keyDown(page, { key: 'ArrowRight', shiftKey: true });
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'seek', positionMs: 70_000 });
    fireEvent.keyDown(page, { key: 's' });
    expect(commands.playbackControl).toHaveBeenLastCalledWith({
      type: 'setShuffle',
      enabled: true,
    });

    // Not while typing.
    const calls = commands.playbackControl.mock.calls.length;
    const field = document.createElement('input');
    document.body.append(field);
    fireEvent.keyDown(field, { key: ' ' });
    expect(commands.playbackControl.mock.calls.length).toBe(calls);
    field.remove();

    fireEvent.keyDown(page, { key: '?' });
    expect(await screen.findByRole('dialog', { name: 'Keyboard shortcuts' })).toHaveTextContent(
      'Play or pause',
    );
  });

  it('sets the speed and the sleep timer from the bar', async () => {
    setWide();
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    await user.click(within(bar).getByRole('button', { name: 'Speed and sleep timer' }));
    const extras = await screen.findByRole('dialog', { name: 'Speed and sleep timer' });
    await user.click(within(extras).getByRole('radio', { name: '1.25×' }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'setRate', rate: 1.25 });
    await user.click(within(extras).getByRole('button', { name: '30 minutes' }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({
      type: 'setSleepTimer',
      timer: { kind: 'minutes', minutes: 30 },
    });
    await user.click(within(extras).getByRole('button', { name: 'End of the song' }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({
      type: 'setSleepTimer',
      timer: { kind: 'endOfTrack' },
    });
  });

  it('saves the queue as a playlist', async () => {
    setWide();
    const user = userEvent.setup();
    commands.playlistCreate.mockResolvedValue(
      ok({ id: 'p9', name: 'Queue', itemCount: 0, artworkUrls: [] }),
    );
    commands.playlistAdd.mockResolvedValue(ok(null));
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;

    await user.click(
      within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Queue' }),
    );
    const panel = screen.getByRole('complementary', { name: 'Queue' });
    await user.click(within(panel).getByRole('button', { name: 'Save as playlist' }));
    await vi.waitFor(() =>
      expect(commands.playlistAdd).toHaveBeenCalledWith('p9', [
        expect.objectContaining({ title: 'Intro' }),
        expect.objectContaining({ title: 'Second' }),
        expect.objectContaining({ title: 'Third' }),
      ]),
    );
    expect(await within(panel).findByText(/^Saved as/)).toBeInTheDocument();
  });
});
