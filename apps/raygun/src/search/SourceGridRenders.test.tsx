import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { sourceExtension } from '../test/sourceExtension';
import { SourceSwitchPanel } from './CustomSources';
import type * as TileModule from './SourceTile';
import { useCustomSources } from './useCustomSources';
import type { SourceOption } from './useCustomSources';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

// Every tile drawn, by its source: shows what a switch or a key redraws.
const drawn = vi.hoisted(() => ({ ids: [] as string[] }));
vi.mock('./SourceTile', async (original) => {
  const actual = await original<typeof TileModule>();
  const { createElement, memo } = await import('react');
  return {
    ...actual,
    SourceTile: memo((props: TileModule.SourceTileProps) => {
      drawn.ids.push(props.id);
      return createElement(actual.SourceTile, props);
    }),
  };
});

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SOURCES: SourceOption[] = Array.from({ length: 40 }, (_, index) => ({
  id: `dev.example.ext${index}/source`,
  name: `Source ${index}`,
  extensionId: `dev.example.ext${index}`,
}));

const Panel: FC = () => {
  const choice = useCustomSources('manga', SOURCES);
  return (
    <SourceSwitchPanel
      open
      id="custom-sources"
      medium="manga"
      sources={SOURCES}
      picked={choice.picked}
      onChange={choice.set}
      onChoose={choice.choose}
    />
  );
};

describe("Custom sources' grid as it changes", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('redraws only the tile switched, and only the tiles a key or None changes', async () => {
    const user = userEvent.setup();
    seedBindings();
    commands.extensionsList.mockResolvedValue(
      ok(
        SOURCES.map((source, index) => ({
          ...sourceExtension(['manga.source'], source.extensionId, source.name),
          repository: 'Paraline',
          icon: `http://127.0.0.1:9/cover/ext-${index}`,
        })),
      ),
    );
    // A wide panel: eight tiles a row.
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(640);
    localStorage.setItem(
      'raygun.search.sources.manga',
      JSON.stringify(SOURCES.map((source) => source.id)),
    );
    renderWithProviders(<Panel />);
    // Settled: the extensions' icons came.
    await waitFor(() =>
      expect(screen.getByRole('switch', { name: 'Source 39' }).querySelector('img')).not.toBeNull(),
    );

    drawn.ids = [];
    await user.click(screen.getByRole('switch', { name: 'Source 12' }));
    expect(screen.getByRole('switch', { name: 'Source 12' })).toHaveAttribute(
      'aria-checked',
      'false',
    );
    expect(drawn.ids).toEqual(['dev.example.ext12/source']);

    // Moving with the keys redraws the tile left and the one reached (the stop for Tab).
    act(() => screen.getByRole('switch', { name: 'Source 0' }).focus());
    drawn.ids = [];
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('switch', { name: 'Source 1' })).toHaveFocus();
    expect(drawn.ids.sort()).toEqual(['dev.example.ext0/source', 'dev.example.ext1/source']);

    // None redraws the tiles it switched off, not the one already off.
    drawn.ids = [];
    await user.click(screen.getByRole('button', { name: 'None' }));
    expect(drawn.ids).toHaveLength(39);
    expect(drawn.ids).not.toContain('dev.example.ext12/source');
  });
});
