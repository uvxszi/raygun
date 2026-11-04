import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import { PrefSize, PrefSlider } from './prefs';
import { parseSize } from './sizes';
import type { SizeFieldProps } from './prefs';

const Size: FC<Omit<SizeFieldProps, 'value' | 'onChange'> & { start: number; saved: number[] }> = ({
  start,
  saved,
  ...props
}) => {
  const [value, setValue] = useState(start);
  return (
    <ul>
      <PrefSize
        {...props}
        value={value}
        onChange={(next) => {
          saved.push(next);
          setValue(next);
        }}
      />
    </ul>
  );
};

describe('Size rows', () => {
  it('read typed numbers on the step and within the limits', () => {
    const limits = { min: -6, max: 12, step: 1 };
    expect(parseSize('3', limits)).toBe(3);
    expect(parseSize('+2.6', limits)).toBe(3);
    expect(parseSize('−4', limits)).toBe(-4);
    expect(parseSize('-40', limits)).toBe(-6);
    expect(parseSize('99', limits)).toBe(12);
    expect(parseSize('', limits)).toBeNull();
    expect(parseSize('big', limits)).toBeNull();
    // Stored as a percentage of the screen's size, shown as what's added to it.
    expect(parseSize('15', { min: 50, max: 200, step: 5, zero: 100 })).toBe(115);
    // Stored in tenths, shown whole.
    expect(parseSize('1,8', { min: 12, max: 22, step: 1, factor: 0.1, zero: 0 })).toBe(18);
  });

  it('take a slider, a typed number and Reset', async () => {
    const user = userEvent.setup();
    const saved: number[] = [];
    render(<Size title="Text size" unit="px" min={-6} max={12} step={1} start={0} saved={saved} />);

    const slider = screen.getByRole('slider', { name: 'Text size' });
    const field = screen.getByRole('spinbutton', { name: 'Text size' });
    const reset = screen.getByRole('button', { name: 'Reset Text size' });
    expect(field).toHaveValue(0);
    expect(reset).toBeDisabled();

    // The browser's `change` comes when the thumb is let go; dragging only moves it.
    fireEvent.change(slider, { target: { value: '4' } });
    expect(saved).toEqual([4]);
    expect(slider).toHaveAttribute('aria-valuetext', '+4 px');
    expect(screen.getByRole('spinbutton', { name: 'Text size' })).toHaveValue(4);

    const typed = screen.getByRole('spinbutton', { name: 'Text size' });
    await user.clear(typed);
    await user.type(typed, '-3{Enter}');
    expect(saved).toEqual([4, -3]);
    expect(screen.getByRole('slider', { name: 'Text size' })).toHaveValue('-3');

    // Past the limit keeps to it.
    const again = screen.getByRole('spinbutton', { name: 'Text size' });
    await user.clear(again);
    await user.type(again, '40');
    await user.tab();
    expect(saved).toEqual([4, -3, 12]);
    expect(screen.getByRole('spinbutton', { name: 'Text size' })).toHaveValue(12);

    await user.click(screen.getByRole('button', { name: 'Reset Text size' }));
    expect(saved).toEqual([4, -3, 12, 0]);
    expect(screen.getByRole('button', { name: 'Reset Text size' })).toBeDisabled();
  });

  it('show stored percentages as what they add to the screen', () => {
    const saved: number[] = [];
    render(
      <Size
        title="Interface size"
        unit="%"
        zero={100}
        min={50}
        max={200}
        step={5}
        start={110}
        saved={saved}
      />,
    );
    expect(screen.getByRole('spinbutton', { name: 'Interface size' })).toHaveValue(10);
    expect(screen.getByRole('slider', { name: 'Interface size' })).toHaveAttribute(
      'aria-valuetext',
      '+10 %',
    );
  });

  it('keep a slider at the value let go until the saved one comes back', () => {
    const saved: number[] = [];
    render(
      <ul>
        <PrefSlider
          title="Line spacing"
          value={16}
          min={12}
          max={22}
          step={1}
          format={(value) => (value / 10).toFixed(1)}
          onChange={(value) => saved.push(value)}
        />
      </ul>,
    );
    const slider = screen.getByRole('slider', { name: 'Line spacing' });
    fireEvent.input(slider, { target: { value: '18' } });
    expect(saved).toEqual([]);
    expect(screen.getByText('1.8')).toBeInTheDocument();
    fireEvent.change(slider, { target: { value: '18' } });
    expect(saved).toEqual([18]);
    expect(screen.getByText('1.8')).toBeInTheDocument();
  });
});
