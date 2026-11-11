import type { AudioOutputs as Outputs } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { AudioOutputs } from './AudioOutputs';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const PHONE: Outputs['outputs'] = [
  { id: '2', name: 'Pixel 4', kind: 'speaker' },
  { id: '7', name: 'Pixel 4', kind: 'wired' },
];

const answer = (overrides: Partial<Outputs> = {}): Outputs => ({
  outputs: PHONE,
  chosen: null,
  routed: null,
  system: null,
  web: false,
  refused: false,
  ...overrides,
});

const rows = async () => within(await screen.findByTestId('audio-outputs'));

describe('AudioOutputs', () => {
  beforeEach(() => {
    seedBindings();
    commands.audioOutputSet.mockResolvedValue({ status: 'ok', data: null });
  });

  it('marks where the playing song goes and where Automatic sends it', async () => {
    commands.audioOutputs.mockResolvedValue(answer({ routed: '7', system: '7' }));
    renderWithProviders(<AudioOutputs />);

    const list = await rows();
    const automatic = list.getByRole('button', { name: /^Automatic/ });
    expect(automatic).toHaveAttribute('aria-pressed', 'true');
    expect(automatic).toHaveTextContent('Now: Wired headphones');
    const wired = list.getByRole('button', { name: /^Wired headphones/ });
    expect(within(wired).getByTestId('output-playing')).toHaveTextContent('Playing here');
    expect(list.getByRole('button', { name: /^Phone speaker/ })).not.toHaveTextContent(
      'Playing here',
    );
    expect(screen.queryByTestId('output-note')).not.toBeInTheDocument();
  });

  it('picks the speaker and follows the song onto it', async () => {
    const user = userEvent.setup();
    commands.audioOutputs.mockResolvedValue(answer({ routed: '7', system: '7' }));
    renderWithProviders(<AudioOutputs />);

    commands.audioOutputs.mockResolvedValue(answer({ chosen: '2', routed: '2', system: '7' }));
    await user.click((await rows()).getByRole('button', { name: /^Phone speaker/ }));

    expect(commands.audioOutputSet).toHaveBeenCalledWith('2');
    const speaker = (await rows()).getByRole('button', { name: /^Phone speaker/ });
    await within(speaker).findByTestId('output-playing');
    expect(speaker).toHaveAttribute('aria-pressed', 'true');
    expect((await rows()).getByRole('button', { name: /^Automatic/ })).not.toHaveTextContent(
      'Now:',
    );
  });

  it('says when Android keeps the sound elsewhere than picked', async () => {
    commands.audioOutputs.mockResolvedValue(
      answer({ chosen: '2', routed: '7', system: '7', refused: true }),
    );
    renderWithProviders(<AudioOutputs />);

    expect(await screen.findByTestId('output-note')).toHaveTextContent(
      'Android kept the sound on Wired headphones',
    );
    const list = await rows();
    expect(list.getByRole('button', { name: /^Phone speaker/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(
      within(list.getByRole('button', { name: /^Wired headphones/ })).getByTestId('output-playing'),
    ).toBeInTheDocument();
  });

  it("says a pick doesn't move YouTube's player", async () => {
    commands.audioOutputs.mockResolvedValue(
      answer({ chosen: '2', routed: '7', system: '7', web: true }),
    );
    renderWithProviders(<AudioOutputs />);

    expect(await screen.findByTestId('output-note')).toHaveTextContent(
      "YouTube's player plays where Android sends sound: Wired headphones",
    );
  });

  it('says so when the picked output is gone', async () => {
    const user = userEvent.setup();
    commands.audioOutputs.mockResolvedValue(answer());
    commands.audioOutputSet.mockResolvedValue({
      status: 'error',
      error: "That output isn't connected any more",
    });
    renderWithProviders(<AudioOutputs />);

    await user.click((await rows()).getByRole('button', { name: /^Wired headphones/ }));
    expect(await screen.findByTestId('output-note')).toHaveTextContent(
      "That output isn't connected any more",
    );
  });
});
