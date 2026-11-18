import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock } from '../test/bindingsMock';
import type { SourceGroup, SourceLink } from './protocol';
import { SourcesPanel } from './SourcesPanel';
import { useVideoPlayerStore } from './videoPlayerStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const option = (height: number, current = false): SourceLink => ({
  label: `${height}p`,
  current,
  failed: false,
  facts: { height } as SourceLink['facts'],
});

const group = (name: string, links: SourceLink[]): SourceGroup => ({
  name,
  state: 'ready',
  error: null,
  links,
  kind: 'addon',
});

describe('the Sources panel', () => {
  beforeEach(() => {
    useVideoPlayerStore.setState({
      request: { id: 'stremio:movie:tt1' } as never,
      sources: [
        {
          ...group('Alpha', [option(2160), option(1080, true), option(1080), option(720)]),
          // Rust's rows: 1080p's two links once (the playing one best), preferred first.
          qualities: [
            { links: [1, 2], preferred: true },
            { links: [0], preferred: false },
            { links: [3], preferred: false },
          ],
        },
        {
          ...group('Beta', [option(1440), option(1080), option(1080)]),
          qualities: [
            { links: [2, 1], preferred: true },
            { links: [0], preferred: false },
          ],
        },
      ],
      sourceOrder: [0, 1],
      usualSource: 'Beta',
    });
  });

  it('tags the preferred option within each source, lists it first and shows a tier once', async () => {
    const user = userEvent.setup();
    render(<SourcesPanel positionMs={1000} onClose={vi.fn()} />);
    expect(screen.queryByRole('group', { name: 'Preferred' })).not.toBeInTheDocument();
    const alpha = screen.getByRole('group', { name: 'Alpha' });
    const rows = within(alpha).getAllByRole('menuitemradio');
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringMatching(/^1080pPreferred.*1 more link: tap to pick/),
      expect.stringMatching(/^2160p \(4K\)/),
      expect.stringMatching(/^720p/),
    ]);
    // Only the source's own options: no tag on another row.
    expect(within(alpha).getAllByTestId('option-badge')).toHaveLength(1);
    expect(screen.getByTestId('sources-usual')).toHaveTextContent('You usually watch from Beta');

    // The row playing, tapped again: its links, to pick one.
    expect(rows[0]).toHaveAttribute('aria-expanded', 'false');
    await user.click(rows[0]!);
    const links = screen.getByRole('menu', { name: 'Links with 1080p' });
    const choices = within(links).getAllByRole('menuitemradio');
    expect(choices).toHaveLength(2);
    await user.click(choices[1]!);
    expect(commands.videoPlayerReport).toHaveBeenCalledWith(
      expect.stringContaining('"action":"source","positionMs":1000,"group":0,"link":2'),
    );
  });

  it('plays the best of a tier’s links on the first tap', async () => {
    const user = userEvent.setup();
    render(<SourcesPanel positionMs={0} onClose={vi.fn()} />);
    await user.click(screen.getByRole('menuitemradio', { name: /^Beta/ }));
    const beta = screen.getByRole('group', { name: 'Beta' });
    await user.click(within(beta).getAllByRole('menuitemradio')[0]!);
    expect(commands.videoPlayerReport).toHaveBeenCalledWith(
      expect.stringContaining('"action":"source","positionMs":0,"group":1,"link":2'),
    );
  });

  it('closes a source’s settings with its own button and with Escape, before the panel', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<SourcesPanel positionMs={0} onClose={onClose} />);
    await user.click(screen.getByRole('button', { name: 'Alpha: settings' }));
    expect(screen.getByText('Turn off everywhere')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Close settings' }));
    expect(screen.queryByText('Turn off everywhere')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Beta: settings' }));
    await user.keyboard('{Escape}');
    expect(screen.queryByText('Turn off everywhere')).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
