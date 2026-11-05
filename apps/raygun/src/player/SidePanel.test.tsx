import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** A window at least `md` wide. */
const setWide = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: true,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

const openPanel = async () => {
  const user = userEvent.setup();
  await AppShellWrapper.mount();
  AppShellWrapper.playing(
    [
      makeItem('Welcome To The Circus', '"Five Finger Death Punch'),
      makeItem('Wrong Side of Heaven', '"Five Finger Death Punch"'),
    ],
    0,
  );
  await AppShellWrapper.miniPlayer.title;
  await user.click(
    within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Now playing view' }),
  );
  return screen.getByRole('complementary', { name: 'Now playing' });
};

describe('Side panel', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    setWide();
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('names the artists without the quotes a source left around them', async () => {
    const panel = await openPanel();

    // The artist line leads to the artist's own page, not to `"Five Finger Death Punch`.
    expect(
      within(panel).getByRole('link', { name: 'Five Finger Death Punch' }),
    ).toBeInTheDocument();
    const nextUp = within(panel).getByRole('region', { name: 'Next in queue' });
    expect(within(nextUp).getByText('Five Finger Death Punch')).toBeInTheDocument();
    expect(panel).not.toHaveTextContent('"Five');
  });

  it('shows the whole title and the tags under the song, apart from its buttons', async () => {
    commands.musicTrackTags.mockResolvedValue({
      status: 'ok',
      data: [
        { name: 'Metal', kind: 'genre' },
        { name: 'Energetic', kind: 'mood' },
      ],
    });
    const panel = await openPanel();

    // The title wraps instead of sliding, and holds its full text for the pointer.
    const title = within(panel).getByTestId('side-panel-title');
    expect(title).toHaveTextContent('Welcome To The Circus');
    expect(title).toHaveAttribute('title', 'Welcome To The Circus');

    const tags = await within(panel).findByRole('list', { name: 'Tags' });
    expect(within(tags).getByRole('link', { name: 'Metal' })).toBeInTheDocument();
    expect(within(tags).getByRole('link', { name: 'Energetic' })).toBeInTheDocument();
    // The buttons sit beside the title, not beside the tags: neither block holds the other.
    const actions = within(panel).getByTestId('side-panel-actions');
    expect(within(actions).getByRole('button', { name: 'Open player' })).toBeInTheDocument();
    expect(actions).not.toContainElement(tags);
    expect(title.parentElement).not.toContainElement(tags);
  });
});
