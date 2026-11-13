import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TimeBar } from './TimeBar';

const setup = (dragOnly: boolean) => {
  const onSeek = vi.fn();
  const onDrag = vi.fn();
  render(
    <TimeBar
      timeMs={60_000}
      durationMs={600_000}
      bufferedMs={0}
      chapters={[]}
      seekStepMs={10_000}
      onSeek={onSeek}
      dragOnly={dragOnly}
      onDrag={onDrag}
    />,
  );
  const bar = screen.getByTestId('time-bar');
  bar.getBoundingClientRect = () =>
    ({ left: 0, top: 0, width: 1_000, height: 20, right: 1_000, bottom: 20 }) as DOMRect;
  return { bar, onSeek, onDrag };
};

describe('TimeBar seeking by dragging', () => {
  it('does nothing on a click', async () => {
    const user = userEvent.setup();
    const { bar, onSeek, onDrag } = setup(true);
    await user.pointer([
      { keys: '[MouseLeft>]', target: bar, coords: { clientX: 800 } },
      { keys: '[/MouseLeft]', target: bar, coords: { clientX: 800 } },
    ]);
    expect(onSeek).not.toHaveBeenCalled();
    expect(onDrag).not.toHaveBeenCalled();
  });

  it('seeks from where the video was by how far it was dragged', async () => {
    const user = userEvent.setup();
    const { bar, onSeek, onDrag } = setup(true);
    await user.pointer([
      { keys: '[MouseLeft>]', target: bar, coords: { clientX: 500 } },
      { target: bar, coords: { clientX: 510 } },
      { target: bar, coords: { clientX: 610 } },
      { keys: '[/MouseLeft]', target: bar, coords: { clientX: 610 } },
    ]);
    expect(onDrag).toHaveBeenCalledWith(120_000);
    expect(onDrag).toHaveBeenLastCalledWith(null);
    expect(onSeek).toHaveBeenCalledWith(120_000);
  });
});
