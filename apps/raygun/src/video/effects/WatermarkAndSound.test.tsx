import type { PictureEffects } from '@raygun/bindings';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import { bindingsMock } from '../../test/bindingsMock';
import { sendRequest } from '../protocol';
import { EffectsPanel } from './EffectsPanel';
import { isNeutral, withEffectDefaults } from './effectParams';
import { useVideoEffects } from './useVideoEffects';
import { useVideoSound } from './videoSound';
import { boxFromDrag } from './watermarkBox';
import { WatermarkDraw } from './WatermarkDraw';
import { NO_WATERMARK } from './watermarkMath';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

type Request = { action: string; change: Record<string, unknown> };

const requests = (): Request[] =>
  commands.videoPlayerReport.mock.calls
    .map(([json]) => JSON.parse(json as string) as Request & { state: string })
    .filter((report) => report.state === 'request');

const BOX = { x: 8_000, y: 500, width: 1_500, height: 1_000 };

const Harness: FC<{ pictureEffects?: PictureEffects }> = ({ pictureEffects }) => {
  const remember = (change: Record<string, unknown>) =>
    sendRequest('movie', 'state', 0, { group: 'movie', change });
  const effects = useVideoEffects({
    titleState: { pictureEffects },
    defaults: {},
    remember,
    savePrefs: (change) => sendRequest('movie', 'prefs', 0, { change }),
  });
  const sound = useVideoSound(null, remember);
  const [speed, setSpeed] = useState(100);
  return (
    <>
      <EffectsPanel
        effects={effects}
        onClose={() => undefined}
        sound={{
          speed,
          onSpeed: setSpeed,
          sound: sound.sound,
          setSound: sound.setSound,
          resetSound: () => {
            sound.resetSound();
            setSpeed(100);
          },
          musicEq: {
            eqEnabled: true,
            eqPreset: 'rock',
            eqBands: [50, 40, 0, 0, 0, 0, 0, 0, 0, 0],
            preamp: 0,
          },
        }}
      />
      <output data-testid="picture">{JSON.stringify(effects.picture)}</output>
      <output data-testid="drawing">{String(effects.drawingWatermark)}</output>
      <output data-testid="speed">{speed}</output>
      <output data-testid="sound">{JSON.stringify(sound.sound)}</output>
    </>
  );
};

const picture = () =>
  JSON.parse(screen.getByTestId('picture').textContent ?? '{}') as ReturnType<
    typeof withEffectDefaults
  >;

describe('the watermark remover in the Effects panel', () => {
  beforeEach(() => commands.videoPlayerReport.mockReset());

  it('is off by default, and a box that is off changes nothing', () => {
    expect(isNeutral(withEffectDefaults({ watermark: { ...BOX, enabled: false } }))).toBe(true);
    expect(isNeutral(withEffectDefaults({ watermark: { ...BOX, enabled: true } }))).toBe(false);
    // On without a box: nothing to take out yet.
    expect(isNeutral(withEffectDefaults({ watermark: { enabled: true } }))).toBe(true);
  });

  it('turns on for the title and asks for a box to be drawn', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('menuitemcheckbox', { name: /Remove a watermark/ }));
    expect(requests().at(-1)).toMatchObject({
      action: 'state',
      change: { pictureEffects: { watermark: { ...NO_WATERMARK, enabled: true } } },
    });
    await user.click(screen.getByRole('button', { name: 'Draw a box' }));
    expect(screen.getByTestId('drawing')).toHaveTextContent('true');
    expect(screen.queryByRole('slider', { name: 'Feather' })).not.toBeInTheDocument();
  });

  it('corrects what is left and keeps the box when a preset is picked', async () => {
    const user = userEvent.setup();
    render(<Harness pictureEffects={{ watermark: { ...BOX, enabled: true } }} />);
    expect(screen.getByText(/Learning the watermark/)).toBeInTheDocument();
    fireEvent.change(screen.getByRole('slider', { name: 'Feather' }), { target: { value: '40' } });
    expect(picture().watermark.feather).toBe(40);
    await user.click(screen.getByRole('button', { name: 'Old film' }));
    expect(picture()).toMatchObject({ deflicker: 60, watermark: { ...BOX, feather: 40 } });
    await user.click(screen.getByRole('button', { name: 'Find it' }));
    expect(screen.getByText(/Looking for a watermark/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Remove the box' }));
    expect(picture().watermark).toMatchObject({ enabled: true, width: 0, height: 0 });
  });

  it('never makes the watermark part of the default', async () => {
    const user = userEvent.setup();
    render(<Harness pictureEffects={{ gamma: 20, watermark: { ...BOX, enabled: true } }} />);
    await user.click(screen.getByRole('button', { name: 'Make default' }));
    const prefs = requests().find((request) => request.action === 'prefs');
    expect(prefs?.change).toMatchObject({ pictureEffects: { gamma: 20, watermark: NO_WATERMARK } });
    // The title keeps its box.
    expect(picture().watermark).toMatchObject(BOX);
  });
});

describe('drawing the watermark box', () => {
  const box = { left: 0, top: 60, width: 640, height: 360, stageWidth: 640, stageHeight: 480 };

  it('maps a drag over the picture to its share of it, kept inside', () => {
    expect(boxFromDrag({ x: 512, y: 78 }, { x: 608, y: 114 }, box)).toEqual({
      x: 8_000,
      y: 500,
      width: 1_500,
      height: 1_000,
    });
    // Dragged past the picture's corner: up to its edge.
    expect(boxFromDrag({ x: 600, y: 400 }, { x: 700, y: 470 }, box)).toMatchObject({
      x: 9_375,
      width: 625,
    });
    expect(boxFromDrag({ x: 10, y: 100 }, { x: 11, y: 101 }, box)).toBeNull();
  });

  it('draws a box with the pointer, and Escape leaves it as it was', () => {
    const done = vi.fn();
    const cancel = vi.fn();
    render(<WatermarkDraw picture={box} current={null} onDone={done} onCancel={cancel} />);
    const layer = screen.getByTestId('watermark-draw');
    fireEvent.pointerDown(layer, { clientX: 512, clientY: 78, pointerId: 1 });
    fireEvent.pointerMove(layer, { clientX: 560, clientY: 100, pointerId: 1 });
    expect(screen.getByTestId('watermark-box')).toBeInTheDocument();
    fireEvent.pointerUp(layer, { clientX: 608, clientY: 114, pointerId: 1 });
    expect(done).toHaveBeenCalledWith({ x: 8_000, y: 500, width: 1_500, height: 1_000 });
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(cancel).toHaveBeenCalled();
  });
});

describe('a title’s sound in the Effects panel', () => {
  beforeEach(() => commands.videoPlayerReport.mockReset());

  it('changes tempo and pitch, keeps the pitch for the title, and resets both', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    fireEvent.change(screen.getByRole('slider', { name: 'Tempo' }), { target: { value: '125' } });
    expect(screen.getByTestId('speed')).toHaveTextContent('125');
    fireEvent.change(screen.getByRole('slider', { name: 'Pitch' }), { target: { value: '-20' } });
    expect(screen.getByText('−2 semitones')).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(requests()).toContainEqual(
        expect.objectContaining({
          change: { videoSound: expect.objectContaining({ pitch: -20 }) },
        }),
      ),
    );
    await user.click(screen.getByRole('button', { name: 'Reset sound' }));
    expect(screen.getByTestId('speed')).toHaveTextContent('100');
    expect(requests().at(-1)).toMatchObject({ change: { videoSound: null } });
  });

  it('follows the music player’s equalizer until the title has its own', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    expect(screen.queryByTestId('equalizer')).not.toBeInTheDocument();
    await user.click(screen.getByRole('menuitemcheckbox', { name: /Own equalizer/ }));
    expect(screen.getByTestId('equalizer')).toBeInTheDocument();
    // It starts from what it followed.
    expect(JSON.parse(screen.getByTestId('sound').textContent ?? '{}')).toMatchObject({
      ownEq: true,
      eqEnabled: true,
      eqPreset: 'rock',
    });
  });
});
