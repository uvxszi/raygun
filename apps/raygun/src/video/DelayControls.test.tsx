import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { DelayControls } from './DelayControls';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

describe('DelayControls', () => {
  it('keeps the typed seconds in the row of steps, between the earlier and later ones', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<DelayControls delayMs={1500} label="Audio delay in seconds" onChange={onChange} />);

    const field = screen.getByRole('textbox', { name: 'Audio delay in seconds' });
    const row = field.closest('div');
    const order = [...(row?.querySelectorAll('button, input') ?? [])].map(
      (control) => control.getAttribute('aria-label') ?? control.textContent,
    );
    expect(order).toEqual(['−1', '−0.1', 'Audio delay in seconds', '+0.1', '+1', 'Reset']);

    await user.click(screen.getByRole('button', { name: '−1' }));
    expect(onChange).toHaveBeenLastCalledWith(500);
    await user.click(screen.getByRole('button', { name: '+0.1' }));
    expect(onChange).toHaveBeenLastCalledWith(1600);
    await user.click(screen.getByRole('button', { name: 'Reset' }));
    expect(onChange).toHaveBeenLastCalledWith(0);

    await user.clear(field);
    await user.type(field, '-2.5{Enter}');
    expect(onChange).toHaveBeenLastCalledWith(-2500);
  });

  it('has nothing to reset at no delay', () => {
    render(<DelayControls delayMs={0} label="Subtitle delay in seconds" onChange={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Reset' })).toBeDisabled();
  });
});
