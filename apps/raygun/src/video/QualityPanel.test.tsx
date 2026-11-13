import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock } from '../test/bindingsMock';
import type { MediaEngine } from './mediaEngine';
import type { VariantInfo } from './protocol';
import { QualityPanel } from './QualityPanel';
import { useVideoPlayerStore } from './videoPlayerStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** An HLS engine with these variants, by id; automatic quality. */
const engineWith = (variants: (VariantInfo & { id: number })[]) => {
  const setQuality = vi.fn();
  const engine = {
    variants: () => variants,
    quality: () => -1,
    setQuality,
  } as unknown as MediaEngine;
  return { engine, setQuality };
};

describe('the Quality panel', () => {
  it('lists the tiers with their pictures’ own sizes and switches to a variant by its size', async () => {
    const user = userEvent.setup();
    // A cropped film's ladder: 1920×816 at two bit rates, and 1280×544.
    const { engine, setQuality } = engineWith([
      { id: 0, width: 1280, height: 544, range: 'sdr', codec: null },
      { id: 1, width: 1920, height: 816, range: 'sdr', codec: null },
      { id: 2, width: 1920, height: 816, range: 'sdr', codec: 'H.264' },
    ]);
    useVideoPlayerStore.setState({
      session: false,
      qualities: [
        {
          key: '1080:sdr',
          height: 1080,
          range: 'sdr',
          codec: 'H.264',
          exact: '1920×816',
          sizes: [{ width: 1920, height: 816 }],
          state: 'variant',
        },
        {
          key: '720:sdr',
          height: 720,
          range: 'sdr',
          codec: null,
          exact: '1280×544',
          sizes: [{ width: 1280, height: 544 }],
          state: 'playing',
        },
      ],
    });
    const onClose = vi.fn();
    render(
      <QualityPanel
        videoKey="url:film"
        positionMs={0}
        engine={engine}
        onClose={onClose}
        onAsked={vi.fn()}
      />,
    );
    // What plays can't be picked again without a session's other sources.
    expect(screen.getByRole('menuitemradio', { name: /720p \(1280×544\)/ })).toBeDisabled();
    await user.click(screen.getByRole('menuitemradio', { name: /1080p \(1920×816\)/ }));
    // The last variant of that size (the higher bit rate): switched here, nothing asked of Rust.
    expect(setQuality).toHaveBeenCalledWith(2);
    expect(onClose).toHaveBeenCalled();
    expect(commands.videoPlayerReport).not.toHaveBeenCalledWith(
      expect.stringContaining('"action":"quality"'),
    );
  });

  it('sets the resolution playing apart, whether it was picked or plays by itself', () => {
    const { engine } = engineWith([]);
    const rows = [
      {
        key: '2160:sdr',
        height: 2160,
        range: 'sdr' as const,
        codec: 'HEVC',
        state: 'otherSource' as const,
        group: 1,
        link: 0,
        source: 'Beta',
      },
      {
        key: '1080:sdr',
        height: 1080,
        range: 'sdr' as const,
        codec: null,
        state: 'playing' as const,
      },
    ];
    // A session's single stream, nothing picked: Automatic is ticked, 1080p is what plays.
    useVideoPlayerStore.setState({ session: true, qualities: rows, qualityWanted: null });
    const { unmount } = render(
      <QualityPanel
        videoKey="k"
        positionMs={0}
        engine={engine}
        onClose={vi.fn()}
        onAsked={vi.fn()}
      />,
    );
    expect(screen.getByRole('menuitemradio', { name: 'Automatic' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    const playing = screen.getByRole('menuitemradio', { name: /1080p/ });
    expect(playing).toHaveAttribute('aria-current', 'true');
    expect(playing).toHaveAttribute('aria-checked', 'false');
    expect(playing).toHaveTextContent('Playing');
    expect(screen.getByRole('menuitemradio', { name: /2160p/ })).not.toHaveAttribute(
      'aria-current',
    );
    unmount();

    // 1080p picked here: it's ticked as well, and Automatic isn't.
    useVideoPlayerStore.setState({ qualityWanted: '1080:sdr' });
    render(
      <QualityPanel
        videoKey="k"
        positionMs={0}
        engine={engine}
        onClose={vi.fn()}
        onAsked={vi.fn()}
      />,
    );
    expect(screen.getByRole('menuitemradio', { name: 'Automatic' })).toHaveAttribute(
      'aria-checked',
      'false',
    );
    const picked = screen.getByRole('menuitemradio', { name: /1080p/ });
    expect(picked).toHaveAttribute('aria-current', 'true');
    expect(picked).toHaveAttribute('aria-checked', 'true');
  });
});
