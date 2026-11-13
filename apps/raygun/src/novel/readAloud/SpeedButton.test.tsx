import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import { HOLD_MS } from '../../music/useHoldPress';
import { nextSpeed, speedLabel } from './speed';
import { SpeedButton } from './SpeedButton';

/** The button with its speed kept, as the reader keeps it in settings. */
const Speed: FC<{ onChange: (speed: number) => void }> = ({ onChange }) => {
  const [speed, setSpeed] = useState(100);
  return (
    <SpeedButton
      speed={speed}
      onChange={(next) => {
        setSpeed(next);
        onChange(next);
      }}
    />
  );
};

const PICKER = 'Choose a reading speed';

describe('The reading-aloud bar’s speed', () => {
  it('steps to the next speed on a tap, after the fastest the slowest', async () => {
    const user = userEvent.setup();
    const changes = vi.fn();
    render(<Speed onChange={changes} />);
    const button = screen.getByRole('button', { name: 'Reading speed: 1.0×' });

    await user.click(button);
    expect(changes).toHaveBeenLastCalledWith(120);
    expect(button).toHaveAccessibleName('Reading speed: 1.2×');
    await user.click(button);
    await user.click(button);
    await user.click(button);
    expect(button).toHaveTextContent('0.8×');
    expect(screen.queryByRole('dialog', { name: PICKER })).not.toBeInTheDocument();
  });

  it('shows speeds to pick when held, without stepping', async () => {
    const user = userEvent.setup();
    const changes = vi.fn();
    render(<Speed onChange={changes} />);
    const button = screen.getByRole('button', { name: 'Reading speed: 1.0×' });

    await user.pointer({ keys: '[TouchA>]', target: button });
    await act(() => new Promise((resolve) => setTimeout(resolve, HOLD_MS + 50)));
    await user.pointer({ keys: '[/TouchA]', target: button });
    const picker = await screen.findByRole('dialog', { name: PICKER });
    // The release after the hold stepped nothing.
    expect(changes).not.toHaveBeenCalled();
    const speeds = within(picker).getByRole('group', { name: 'Speeds' });
    expect(
      within(speeds)
        .getAllByRole('button')
        .map((choice) => choice.textContent),
    ).toEqual(['0.5×', '0.75×', '1.0×', '1.25×', '1.5×', '2.0×', '2.5×', '3.0×']);
    expect(within(speeds).getByRole('button', { name: '1.0×' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    await user.click(within(speeds).getByRole('button', { name: '1.25×' }));
    expect(changes).toHaveBeenLastCalledWith(125);
    expect(screen.queryByRole('dialog', { name: PICKER })).not.toBeInTheDocument();
    expect(button).toHaveTextContent('1.25×');
  });

  it('opens on a right click too, with a slider for any speed from 0.5× to 3×', async () => {
    const user = userEvent.setup();
    const changes = vi.fn();
    render(<Speed onChange={changes} />);
    const button = screen.getByRole('button', { name: 'Reading speed: 1.0×' });

    await user.pointer({ keys: '[MouseRight]', target: button });
    const picker = await screen.findByRole('dialog', { name: PICKER });
    const slider = within(picker).getByRole('slider', { name: 'Reading speed' });
    expect(slider).toHaveAttribute('min', '50');
    expect(slider).toHaveAttribute('max', '300');
    expect(slider).toHaveAttribute('step', '5');
    fireEvent.change(slider, { target: { value: '235' } });
    expect(changes).toHaveBeenLastCalledWith(235);
    expect(button).toHaveTextContent('2.35×');

    // A tap on the button puts the speeds away rather than stepping.
    changes.mockClear();
    await user.click(button);
    expect(screen.queryByRole('dialog', { name: PICKER })).not.toBeInTheDocument();
    expect(changes).not.toHaveBeenCalled();
  });
});

describe('Reading speeds', () => {
  it('step through the bar’s speeds', () => {
    expect([80, 100, 125, 150, 200, 250].map(nextSpeed)).toEqual([100, 120, 150, 200, 80, 80]);
  });

  it('show tenths with one decimal and the rest with two', () => {
    expect([50, 75, 100, 125, 300].map(speedLabel)).toEqual([
      '0.5×',
      '0.75×',
      '1.0×',
      '1.25×',
      '3.0×',
    ]);
  });
});
