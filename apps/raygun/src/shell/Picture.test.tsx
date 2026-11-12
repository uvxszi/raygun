import { act, fireEvent, render, screen } from '@testing-library/react';

import { forgetShownImages, rememberShown } from './imageMemory';
import { Picture } from './Picture';

/** Pictures being decoded, by address: the test says when each is ready. */
let decoding: Map<string, () => void>;

/** jsdom decodes nothing: the browser's `decode()` is stood in for. */
const ownDecode = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'decode');

const decoded = async (src: string) => {
  const done = decoding.get(src);
  expect(done, `${src} is being decoded`).toBeDefined();
  await act(async () => done!());
};

const frame = () => screen.getByTestId('frame');
/** The picture shown or on its way. */
const top = () => screen.getByRole('img', { name: 'Cover' });
/** The picture kept under it until it's covered. */
const under = () => frame().querySelector('img[data-under]');
/** The picture fading out over the placeholder. */
const leaving = () => frame().querySelector('img[data-leaving]');
const placeholder = () => screen.queryByText('No cover');

const Cover = ({ src, onFailed }: { src: string | null; onFailed?: (src: string) => void }) => (
  <Picture
    src={src}
    alt="Cover"
    frameTestId="frame"
    placeholder={<span>No cover</span>}
    onFailed={onFailed}
    className="size-10"
  />
);

describe('pictures', () => {
  beforeEach(() => {
    forgetShownImages();
    decoding = new Map();
    document.documentElement.dataset.motion = 'lively';
    Object.defineProperty(HTMLImageElement.prototype, 'decode', {
      configurable: true,
      writable: true,
      value(this: HTMLImageElement) {
        return new Promise<void>((resolve) => {
          decoding.set(this.getAttribute('src') ?? '', resolve);
        });
      },
    });
  });

  afterEach(() => {
    delete document.documentElement.dataset.motion;
    if (ownDecode) Object.defineProperty(HTMLImageElement.prototype, 'decode', ownDecode);
    else delete (HTMLImageElement.prototype as { decode?: unknown }).decode;
  });

  it('fades a new picture in only once it has loaded and decoded', async () => {
    render(<Cover src="http://img/a" />);
    expect(top()).toHaveAttribute('data-fade');
    expect(top()).toHaveAttribute('decoding', 'async');
    expect(top()).not.toHaveAttribute('data-loaded');

    fireEvent.load(top());
    // Loaded, still decoding: it isn't shown half drawn.
    expect(top()).not.toHaveAttribute('data-loaded');
    await decoded('http://img/a');
    expect(top()).toHaveAttribute('data-loaded');
    expect(placeholder()).not.toBeInTheDocument();
  });

  it('shows a picture seen before at once, without fading it in again', () => {
    rememberShown('http://img/seen');
    render(<Cover src="http://img/seen" />);
    expect(top()).not.toHaveAttribute('data-fade');
    expect(top()).toHaveAttribute('decoding', 'sync');
    expect(top()).toHaveAttribute('loading', 'eager');
  });

  it('keeps the picture shown under a new one until the new one has decoded and faded in', async () => {
    const { rerender } = render(<Cover src="http://img/a" />);
    const first = top();
    fireEvent.load(first);
    await decoded('http://img/a');

    rerender(<Cover src="http://img/b" />);
    // The same element stays, under the new picture, which starts transparent.
    expect(under()).toBe(first);
    expect(under()).toHaveAttribute('src', 'http://img/a');
    expect(top()).toHaveAttribute('src', 'http://img/b');
    expect(top()).toHaveAttribute('data-fade');

    fireEvent.load(top());
    expect(under()).toBe(first);
    await decoded('http://img/b');
    expect(top()).toHaveAttribute('data-loaded');
    // Fading in over it: the old one goes once the fade is over.
    expect(under()).toBe(first);
    fireEvent.transitionEnd(top());
    expect(under()).toBeNull();
  });

  it('keeps the placeholder under a picture that comes later, as a cover found elsewhere', async () => {
    const { rerender } = render(<Cover src={null} />);
    expect(placeholder()).toBeInTheDocument();

    rerender(<Cover src="http://img/found" />);
    expect(placeholder()).toBeInTheDocument();
    fireEvent.load(top());
    await decoded('http://img/found');
    fireEvent.transitionEnd(top());
    expect(placeholder()).not.toBeInTheDocument();
  });

  it('fades a picture that is gone out over the placeholder', async () => {
    const { rerender } = render(<Cover src="http://img/a" />);
    fireEvent.load(top());
    await decoded('http://img/a');

    rerender(<Cover src={null} />);
    expect(placeholder()).toBeInTheDocument();
    expect(leaving()).toHaveAttribute('src', 'http://img/a');
    expect(screen.queryByRole('img', { name: 'Cover' })).not.toBeInTheDocument();
    fireEvent.transitionEnd(leaving()!);
    expect(leaving()).toBeNull();
  });

  it('swaps at once where things may not move', async () => {
    document.documentElement.dataset.motion = 'reduced';
    const { rerender } = render(<Cover src="http://img/a" />);
    fireEvent.load(top());
    await decoded('http://img/a');

    rerender(<Cover src="http://img/b" />);
    expect(under()).toHaveAttribute('src', 'http://img/a');
    fireEvent.load(top());
    // Still never half drawn, but no fade to wait for: the old one goes as the new one shows.
    await decoded('http://img/b');
    expect(top()).toHaveAttribute('data-loaded');
    expect(under()).toBeNull();

    rerender(<Cover src={null} />);
    expect(leaving()).toBeNull();
    expect(placeholder()).toBeInTheDocument();
  });

  it('shows the placeholder for a picture that fails, and tries a new address', async () => {
    const onFailed = vi.fn();
    const { rerender } = render(<Cover src="http://img/a" onFailed={onFailed} />);
    fireEvent.load(top());
    await decoded('http://img/a');

    rerender(<Cover src="http://img/broken" onFailed={onFailed} />);
    fireEvent.error(top());
    expect(onFailed).toHaveBeenCalledWith('http://img/broken');
    expect(screen.queryByRole('img', { name: 'Cover' })).not.toBeInTheDocument();
    expect(placeholder()).toBeInTheDocument();
    // The picture shown before fades out over the placeholder rather than staying for this one.
    expect(under()).toBeNull();
    expect(leaving()).toHaveAttribute('src', 'http://img/a');

    rerender(<Cover src="http://img/other" onFailed={onFailed} />);
    expect(top()).toHaveAttribute('src', 'http://img/other');
    // The one that failed stays failed.
    rerender(<Cover src="http://img/broken" onFailed={onFailed} />);
    expect(screen.queryByRole('img', { name: 'Cover' })).not.toBeInTheDocument();
  });
});
