import { render, screen, within } from '@testing-library/react';
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
  const keepQuality = vi.fn(() => true);
  const engine = {
    variants: () => variants,
    quality: () => -1,
    setQuality,
    keepQuality,
  } as unknown as MediaEngine;
  return { engine, setQuality, keepQuality };
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
    await user.click(screen.getByRole('menuitemradio', { name: /1080p \(1920×816\)/ }));
    // The last variant of that size (the higher bit rate): switched here, nothing asked of Rust.
    expect(setQuality).toHaveBeenCalledWith(2);
    expect(onClose).toHaveBeenCalled();
    expect(commands.videoPlayerReport).not.toHaveBeenCalledWith(
      expect.stringContaining('"action":"quality"'),
    );
  });

  it('holds the row playing while automatic, without loading anything again', async () => {
    const user = userEvent.setup();
    const { engine, setQuality, keepQuality } = engineWith([
      { id: 0, width: 1280, height: 720, range: 'sdr', codec: null },
      { id: 1, width: 1920, height: 1080, range: 'sdr', codec: null },
    ]);
    useVideoPlayerStore.setState({
      session: true,
      qualityWanted: null,
      qualities: [
        { key: '1080:sdr', height: 1080, range: 'sdr', codec: null, state: 'variant' },
        { key: '720:sdr', height: 720, range: 'sdr', codec: null, state: 'playing' },
      ],
    });
    const onClose = vi.fn();
    const onAsked = vi.fn();
    render(
      <QualityPanel
        videoKey="k"
        positionMs={5000}
        engine={engine}
        onClose={onClose}
        onAsked={onAsked}
      />,
    );
    await user.click(screen.getByRole('menuitemradio', { name: /720p/ }));
    expect(keepQuality).toHaveBeenCalled();
    expect(setQuality).not.toHaveBeenCalled();
    // The session keeps the step; no source is asked to load.
    expect(onAsked).not.toHaveBeenCalled();
    expect(commands.videoPlayerReport).toHaveBeenCalledWith(expect.stringContaining('"keep":true'));
    expect(onClose).toHaveBeenCalled();
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

  it('tags the preferred row, and lists its links when the row picked is tapped again', async () => {
    const user = userEvent.setup();
    const { engine } = engineWith([]);
    const link = (label: string, height: number, current = false) => ({
      label,
      current,
      failed: false,
      facts: { height, range: 'sdr', codec: 'HEVC', sizeBytes: 2_000_000_000 } as never,
    });
    useVideoPlayerStore.setState({
      session: true,
      qualityWanted: '1080:sdr',
      sources: [
        { name: 'Alpha', state: 'ready', error: null, links: [link('A 1080p', 1080, true)] },
        { name: 'Beta', state: 'ready', error: null, links: [link('B 1080p WEB', 1080)] },
      ],
      qualities: [
        {
          key: '1080:sdr',
          height: 1080,
          range: 'sdr',
          codec: null,
          state: 'playing',
          preferred: true,
          alternatives: [
            { group: 0, link: 0 },
            { group: 1, link: 0 },
          ],
        },
        {
          key: '2160:sdr',
          height: 2160,
          range: 'sdr',
          codec: null,
          state: 'otherSource',
          group: 1,
          link: 1,
          source: 'Beta',
        },
      ],
    });
    const onClose = vi.fn();
    render(
      <QualityPanel
        videoKey="k"
        positionMs={3000}
        engine={engine}
        onClose={onClose}
        onAsked={vi.fn()}
      />,
    );
    const rows = screen.getAllByRole('menuitemradio');
    // Automatic, then the preferred row (tagged), then the rest.
    expect(rows[1]).toHaveTextContent(/^1080pPreferred/);
    expect(screen.getAllByTestId('option-badge')).toHaveLength(1);
    await user.click(rows[1]!);
    expect(onClose).not.toHaveBeenCalled();
    const links = screen.getByRole('menu', { name: 'Links with 1080p' });
    const choices = within(links).getAllByRole('menuitemradio');
    expect(choices.map((choice) => choice.textContent)).toEqual([
      expect.stringContaining('1080p · Alpha'),
      expect.stringMatching(/1080p · Beta.*HEVC.*2\.0 GB.*B 1080p WEB/),
    ]);
    await user.click(choices[1]!);
    expect(commands.videoPlayerReport).toHaveBeenCalledWith(
      expect.stringContaining('"action":"source","positionMs":3000,"group":1,"link":0'),
    );
    expect(onClose).toHaveBeenCalled();
  });
});
