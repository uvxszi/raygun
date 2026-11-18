import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import '../i18n';
import { ScreenshotQueue } from './ScreenshotQueue';
import { sortByTaken } from './screenshotOrder';

const shot = (name: string, lastModified: number) =>
  new File(['x'], name, { type: 'image/png', lastModified });

const Harness: FC<{ initial: File[]; onRead: (files: File[]) => void }> = ({ initial, onRead }) => {
  const [files, setFiles] = useState(initial);
  return (
    <>
      <ScreenshotQueue files={files} onChange={setFiles} />
      <button type="button" onClick={() => onRead(files)}>
        Read
      </button>
    </>
  );
};

describe('ScreenshotQueue (M46.40)', () => {
  beforeAll(() => {
    URL.createObjectURL = vi.fn(() => 'blob:preview');
    URL.revokeObjectURL = vi.fn();
  });

  it('orders screenshots by when they were taken', () => {
    const sorted = sortByTaken([
      shot('b.png', 20),
      shot('a.png', 10),
      shot('c 10.png', 30),
      shot('c 2.png', 30),
    ]);
    expect(sorted.map((file) => file.name)).toEqual(['a.png', 'b.png', 'c 2.png', 'c 10.png']);
  });

  it('moves and removes screenshots, and reads them in the shown order', async () => {
    const user = userEvent.setup();
    const onRead = vi.fn();
    render(
      <Harness
        initial={[shot('one.png', 1), shot('two.png', 2), shot('three.png', 3)]}
        onRead={onRead}
      />,
    );
    const queue = screen.getByTestId('screenshot-queue');
    await user.click(within(queue).getByRole('button', { name: 'Move three.png up' }));
    await user.click(within(queue).getByRole('button', { name: 'Remove one.png' }));
    await user.click(screen.getByRole('button', { name: 'Read' }));
    expect(onRead.mock.calls[0]?.[0].map((file: File) => file.name)).toEqual([
      'three.png',
      'two.png',
    ]);
  });

  it('opens a screenshot before reading to turn or crop it', async () => {
    const user = userEvent.setup();
    const onRead = vi.fn();
    vi.stubGlobal(
      'createImageBitmap',
      vi.fn(async () => ({ width: 40, height: 80, close: () => undefined })),
    );
    const getContext = vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
      drawImage: () => undefined,
      translate: () => undefined,
      rotate: () => undefined,
    } as unknown as CanvasRenderingContext2D);
    const toBlob = vi
      .spyOn(HTMLCanvasElement.prototype, 'toBlob')
      .mockImplementation((done) => done(new Blob(['y'], { type: 'image/png' })));
    render(<Harness initial={[shot('one.jpg', 1), shot('two.png', 2)]} onRead={onRead} />);
    await user.click(screen.getByRole('button', { name: 'Open one.jpg' }));
    const inspector = screen.getByRole('dialog', { name: 'Screenshot 1' });
    await user.click(within(inspector).getByRole('button', { name: 'Rotate' }));
    // Cropping keeps the part in the queue instead of reading it.
    await user.click(within(inspector).getByRole('button', { name: 'Crop' }));
    expect(within(inspector).getByRole('button', { name: 'Keep this part' })).toBeDisabled();
    await user.click(within(inspector).getByRole('button', { name: 'Close' }));
    await user.click(screen.getByRole('button', { name: 'Read' }));
    const read: File[] = onRead.mock.calls[0]?.[0] ?? [];
    expect(read.map((file) => file.name)).toEqual(['one.png', 'two.png']);
    // The turned screenshot keeps its time, so its place.
    expect(read[0]?.lastModified).toBe(1);
    getContext.mockRestore();
    toBlob.mockRestore();
    vi.unstubAllGlobals();
  });
});
