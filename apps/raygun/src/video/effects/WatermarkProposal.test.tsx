import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { WatermarkProposal } from './WatermarkProposal';

describe('the watermark Find it found', () => {
  it('is outlined and asked about; only Remove turns the remover on', async () => {
    const user = userEvent.setup();
    const onAnswer = vi.fn();
    render(
      <WatermarkProposal
        picture={{ left: 0, top: 0, width: 1000, height: 500, stageWidth: 1000, stageHeight: 500 }}
        box={{ x: 8_000, y: 500, width: 1_500, height: 1_000 }}
        onAnswer={onAnswer}
      />,
    );
    const outline = screen.getByTestId('watermark-proposal-box');
    expect(outline.style.left).toBe('800px');
    expect(outline.style.width).toBe('150px');
    expect(screen.getByRole('dialog', { name: 'Remove this?' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Not this' }));
    expect(onAnswer).toHaveBeenLastCalledWith(false);
    await user.click(screen.getByRole('button', { name: 'Remove' }));
    expect(onAnswer).toHaveBeenLastCalledWith(true);
  });
});
