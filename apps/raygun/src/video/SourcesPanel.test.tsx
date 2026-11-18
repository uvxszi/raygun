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
        group('Alpha', [option(2160, true), option(1080)]),
        group('Beta', [option(1440), option(1080)]),
      ],
      sourceOrder: [0, 1],
      preferredOptions: {
        height: 1080,
        options: [
          { group: 0, link: 1 },
          { group: 1, link: 1 },
        ],
      },
      usualSource: 'Beta',
    });
  });

  it('lists the preferred resolution’s options from every source first', async () => {
    const user = userEvent.setup();
    render(<SourcesPanel positionMs={1000} onClose={vi.fn()} />);
    const preferred = screen.getByRole('group', { name: 'Preferred' });
    const rows = within(preferred).getAllByRole('menuitemradio');
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining('1080p · Alpha'),
      expect.stringContaining('1080p · Beta'),
    ]);
    expect(screen.getByTestId('sources-usual')).toHaveTextContent('You usually watch from Beta');
    await user.click(rows[1]!);
    expect(commands.videoPlayerReport).toHaveBeenCalledWith(
      expect.stringContaining('"action":"source","positionMs":1000,"group":1,"link":1'),
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
