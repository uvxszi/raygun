import { act, render, screen } from '@testing-library/react';
import type { FC } from 'react';

import { useSteadyPicture } from './steadyPicture';

/** A reader page as `ReaderImage` draws it: one picture element, whatever it shows. */
const Page: FC<{ page: string; wanted: string; onScreen?: boolean }> = ({
  page,
  wanted,
  onScreen = true,
}) => <img alt="Page" src={useSteadyPicture(page, wanted, onScreen)} />;

/** Pictures being decoded, by address: the test says when each is ready, or fails. */
let decoding: Map<string, { resolve: () => void; reject: () => void }>;

const finish = async (src: string, ok = true) => {
  const pending = decoding.get(src);
  expect(pending, `${src} is being decoded`).toBeDefined();
  await act(async () => (ok ? pending!.resolve() : pending!.reject()));
};

/** jsdom decodes nothing: the browser's `decode()` is stood in for. */
const ownDecode = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'decode');

describe('reader pages drawn again', () => {
  beforeEach(() => {
    decoding = new Map();
    Object.defineProperty(HTMLImageElement.prototype, 'decode', {
      configurable: true,
      writable: true,
      value(this: HTMLImageElement) {
        return new Promise<void>((resolve, reject) => {
          decoding.set(this.getAttribute('src') ?? '', { resolve, reject: () => reject() });
        });
      },
    });
  });

  afterEach(() => {
    if (ownDecode) Object.defineProperty(HTMLImageElement.prototype, 'decode', ownDecode);
    else delete (HTMLImageElement.prototype as { decode?: unknown }).decode;
  });

  it("keeps the page on screen until an AI layer's picture is decoded, on the same element", async () => {
    const { rerender } = render(<Page page="p1" wanted="http://img/1" />);
    const picture = screen.getByRole('img', { name: 'Page' });
    expect(picture).toHaveAttribute('src', 'http://img/1');

    // An upscaled picture came: the page as it came stays until it's decoded.
    rerender(<Page page="p1" wanted="blob:upscaled" />);
    expect(picture).toHaveAttribute('src', 'http://img/1');
    await finish('blob:upscaled');
    expect(screen.getByRole('img', { name: 'Page' })).toBe(picture);
    expect(picture).toHaveAttribute('src', 'blob:upscaled');

    // Layers off: the drawn picture stays until the page as it came is ready again.
    rerender(<Page page="p1" wanted="http://img/1" />);
    expect(picture).toHaveAttribute('src', 'blob:upscaled');
    await finish('http://img/1');
    expect(picture).toHaveAttribute('src', 'http://img/1');
  });

  it('keeps what shows while newer pictures replace one still decoding', async () => {
    const { rerender } = render(<Page page="p1" wanted="http://img/1" />);
    rerender(<Page page="p1" wanted="blob:upscaled" />);
    rerender(<Page page="p1" wanted="blob:restored" />);
    const picture = screen.getByRole('img', { name: 'Page' });
    expect(picture).toHaveAttribute('src', 'http://img/1');
    // The first one finishing late changes nothing.
    await finish('blob:upscaled');
    expect(picture).toHaveAttribute('src', 'http://img/1');
    await finish('blob:restored');
    expect(picture).toHaveAttribute('src', 'blob:restored');
  });

  it('shows another page, a page not on screen yet, or one that fails to decode at once', async () => {
    const { rerender } = render(<Page page="p1" wanted="http://img/1" />);
    rerender(<Page page="p2" wanted="http://img/2" />);
    expect(screen.getByRole('img', { name: 'Page' })).toHaveAttribute('src', 'http://img/2');
    expect(decoding.size).toBe(0);

    // Still loading (or failed): there's nothing on screen to keep.
    rerender(<Page page="p2" wanted="blob:upscaled" onScreen={false} />);
    expect(screen.getByRole('img', { name: 'Page' })).toHaveAttribute('src', 'blob:upscaled');

    // A picture that can't be decoded swaps in anyway, so the page's own failure shows.
    rerender(<Page page="p2" wanted="blob:broken" />);
    expect(screen.getByRole('img', { name: 'Page' })).toHaveAttribute('src', 'blob:upscaled');
    await finish('blob:broken', false);
    expect(screen.getByRole('img', { name: 'Page' })).toHaveAttribute('src', 'blob:broken');
  });
});
