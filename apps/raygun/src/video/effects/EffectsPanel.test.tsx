import type { PictureEffects, SmartAudio } from '@raygun/bindings';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { bindingsMock } from '../../test/bindingsMock';
import { sendRequest } from '../protocol';
import { EffectsPanel, SmartAudioOptions } from './EffectsPanel';
import { NO_EFFECTS } from './effectParams';
import { useVideoEffects } from './useVideoEffects';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

type Requests = { action: string; change: Record<string, unknown> }[];

/** What the panel asked Rust to keep: `state` (the title's) and `prefs` (the settings'). */
const requests = (): Requests =>
  commands.videoPlayerReport.mock.calls
    .map(([json]) => JSON.parse(json as string) as Requests[number] & { state: string })
    .filter((report) => report.state === 'request');

const Harness: FC<{
  titleState?: { pictureEffects?: PictureEffects; smartAudio?: SmartAudio };
  defaults?: { pictureEffects?: PictureEffects; smartAudio?: SmartAudio };
}> = ({ titleState = {}, defaults = {} }) => {
  const effects = useVideoEffects({
    titleState,
    defaults,
    remember: (change) => sendRequest('movie', 'state', 0, { group: 'movie', change }),
    savePrefs: (change) => sendRequest('movie', 'prefs', 0, { change }),
  });
  return (
    <>
      <EffectsPanel effects={effects} onClose={() => undefined} />
      <SmartAudioOptions effects={effects} />
      <output data-testid="applied">{JSON.stringify(effects.applied)}</output>
    </>
  );
};

const applied = () =>
  JSON.parse(screen.getByTestId('applied').textContent ?? '{}') as typeof NO_EFFECTS;

describe('the Effects panel', () => {
  beforeEach(() => {
    commands.videoPlayerReport.mockReset();
  });

  it('changes the picture as a slider moves and keeps it for the title once it settles', async () => {
    render(<Harness />);
    fireEvent.change(screen.getByRole('slider', { name: 'Contrast' }), { target: { value: '30' } });
    expect(screen.getByText('+30')).toBeInTheDocument();
    expect(applied()).toMatchObject({ contrast: 30 });
    // Not on every step of the slider: once it stops.
    expect(requests()).toEqual([]);
    await vi.waitFor(() =>
      expect(requests()).toContainEqual(
        expect.objectContaining({
          action: 'state',
          change: { pictureEffects: { ...NO_EFFECTS, contrast: 30 } },
        }),
      ),
    );

    await userEvent.setup().click(screen.getByRole('button', { name: 'Reset Contrast' }));
    expect(applied()).toMatchObject({ contrast: 0 });
  });

  it('applies presets and looks, and puts everything back with Reset all', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Old film' }));
    expect(screen.getByRole('button', { name: 'Old film' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(applied()).toMatchObject({ deflicker: 60, denoise: 35, sharpen: 15, contrast: 10 });
    await user.click(screen.getByRole('button', { name: 'Sepia' }));
    expect(applied()).toMatchObject({ look: 'sepia', deflicker: 60 });

    await user.click(screen.getByRole('button', { name: 'Reset all' }));
    expect(applied()).toEqual(NO_EFFECTS);
    // With no effects in the settings either, the title simply follows them again.
    expect(requests().at(-1)).toMatchObject({ action: 'state', change: { pictureEffects: null } });
  });

  it("saves a title's effects as the default, and offers the default back", async () => {
    const user = userEvent.setup();
    render(
      <Harness
        titleState={{ pictureEffects: { gamma: 20 } }}
        defaults={{ pictureEffects: { sharpen: 10 } }}
      />,
    );
    expect(applied()).toMatchObject({ gamma: 20, sharpen: 0 });
    await user.click(screen.getByRole('button', { name: 'Use the default' }));
    expect(applied()).toMatchObject({ gamma: 0, sharpen: 10 });
    expect(screen.queryByRole('button', { name: 'Use the default' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Brighter' }));
    await user.click(screen.getByRole('button', { name: 'Make default' }));
    expect(requests()).toContainEqual(
      expect.objectContaining({
        action: 'prefs',
        change: { pictureEffects: { ...NO_EFFECTS, brightness: 5, gamma: 30 } },
      }),
    );
    expect(requests().at(-1)).toMatchObject({ action: 'state', change: { pictureEffects: null } });
    // Shown as the settings' at once, before Rust's update with them comes.
    expect(applied()).toMatchObject({ brightness: 5, gamma: 30 });
  });

  it('turns smart audio on for the title, and makes it the default', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const night = screen.getByRole('menuitemcheckbox', { name: /Night mode/ });
    expect(night).toHaveAttribute('aria-checked', 'false');
    await user.click(night);
    expect(screen.getByRole('menuitemcheckbox', { name: /Night mode/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(requests().at(-1)).toMatchObject({
      action: 'state',
      change: { smartAudio: { dialogue: false, night: true, normalize: false, leveling: false } },
    });

    await user.click(screen.getByRole('menuitemradio', { name: 'Make default' }));
    expect(requests()).toContainEqual(
      expect.objectContaining({
        action: 'prefs',
        change: { smartAudio: expect.objectContaining({ night: true }) },
      }),
    );
  });
});
