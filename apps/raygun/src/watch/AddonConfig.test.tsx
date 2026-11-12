import type { DetectedSource, StremioAddon, StremioConfigField } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const field = (overrides: Partial<StremioConfigField>): StremioConfigField => ({
  key: 'apiKey',
  kind: 'text',
  title: 'API key',
  default: null,
  options: [],
  required: false,
  value: null,
  ...overrides,
});

/** Settings as an SDK addon declares them: one required, the rest optional. */
const CONFIG: StremioConfigField[] = [
  field({ key: 'apiKey', title: 'API key', required: true }),
  field({ key: 'region', title: 'Region (optional)' }),
  field({ key: 'limit', kind: 'number', title: 'Limit', default: '5' }),
  field({ key: 'hd', kind: 'checkbox', title: 'HD only', default: 'checked' }),
  field({ key: 'quality', kind: 'select', title: 'Quality', options: ['1080p', '720p'] }),
];

const addon = (overrides: Partial<StremioAddon> = {}): StremioAddon => ({
  id: 'films',
  url: 'https://films.example/manifest.json',
  name: 'Open Films',
  description: 'Streams for films',
  logo: null,
  version: '1.0.0',
  resources: ['stream'],
  types: ['movie'],
  catalogs: 0,
  adult: false,
  error: null,
  enabled: true,
  provides: [{ resource: 'stream', kind: null, count: 1 }],
  p2p: false,
  configureUrl: null,
  needsSetup: false,
  failing: false,
  ...overrides,
});

const detected = (found: StremioAddon): DetectedSource => ({
  kind: 'addon',
  format: null,
  server: null,
  url: found.url,
  name: found.name,
  description: null,
  added: false,
  addon: found,
  entries: [],
  note: null,
});

const openAddons = async () => {
  await AppShellWrapper.mount('/sources');
  return screen.findByTestId('sources-page');
};

describe('Stremio addon setup and addons not for Raygun', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('sets an added addon up in its own form, where only required settings must be filled in', async () => {
    const user = userEvent.setup();
    commands.stremioAddons.mockResolvedValue(ok([addon({ config: CONFIG })]));
    commands.stremioAddConfigured.mockResolvedValue(ok(addon({ config: CONFIG })));
    const page = await openAddons();

    const item = await within(page).findByRole('listitem', { name: 'Open Films' });
    await user.click(within(item).getByRole('button', { name: 'More for Open Films' }));
    await user.click(screen.getByRole('menuitem', { name: 'Set up' }));
    expect(commands.stremioConfigure).not.toHaveBeenCalled();
    const sheet = await screen.findByRole('dialog', { name: 'Set up Open Films' });

    const save = within(sheet).getByRole('button', { name: 'Save' });
    expect(save).toBeDisabled();
    expect(within(sheet).getByText('API key (required)')).toBeInTheDocument();
    expect(within(sheet).getByRole('textbox', { name: 'Region (optional)' })).toHaveValue('');
    expect(within(sheet).getByRole('spinbutton', { name: 'Limit' })).toHaveValue(5);
    expect(within(sheet).getByRole('checkbox', { name: 'HD only' })).toBeChecked();
    expect(within(sheet).getByRole('combobox', { name: 'Quality' })).toHaveValue('1080p');

    // The optional settings stay empty: only the required one is filled in.
    await user.type(within(sheet).getByRole('textbox', { name: 'API key' }), 'secret-key');
    expect(save).toBeEnabled();
    await user.click(within(sheet).getByRole('checkbox', { name: 'HD only' }));
    await user.click(save);

    expect(commands.stremioAddConfigured).toHaveBeenCalledWith(
      'https://films.example/manifest.json',
      [
        { key: 'apiKey', value: 'secret-key' },
        { key: 'region', value: '' },
        { key: 'limit', value: '5' },
        { key: 'hd', value: '' },
        { key: 'quality', value: '1080p' },
      ],
    );
    await vi.waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Set up Open Films' })).toBeNull(),
    );
  });

  it('starts from the setup in its address and says what Rust refused', async () => {
    const user = userEvent.setup();
    const configured = CONFIG.map((setting) =>
      setting.key === 'apiKey'
        ? { ...setting, value: 'old-key' }
        : setting.key === 'hd'
          ? { ...setting, value: '' }
          : setting,
    );
    commands.stremioAddons.mockResolvedValue(ok([addon({ config: configured })]));
    commands.stremioAddConfigured.mockResolvedValue({
      status: 'error',
      error: 'Limit takes a number',
    });
    const page = await openAddons();

    const item = await within(page).findByRole('listitem', { name: 'Open Films' });
    await user.click(within(item).getByRole('button', { name: 'More for Open Films' }));
    await user.click(screen.getByRole('menuitem', { name: 'Set up' }));
    const sheet = await screen.findByRole('dialog', { name: 'Set up Open Films' });
    expect(within(sheet).getByRole('textbox', { name: 'API key' })).toHaveValue('old-key');
    expect(within(sheet).getByRole('checkbox', { name: 'HD only' })).not.toBeChecked();

    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    expect(await within(sheet).findByText('Limit takes a number')).toBeInTheDocument();
  });

  it('marks an added addon that does nothing in Raygun, with Remove in place of its switch', async () => {
    const user = userEvent.setup();
    commands.stremioAddons.mockResolvedValue(
      ok([
        addon(),
        addon({
          id: 'lists',
          url: 'https://lists.example/manifest.json',
          name: 'Addon Lists',
          resources: ['addon_catalog'],
          provides: [],
          notForRaygun:
            "Only lists other Stremio addons, which Raygun doesn't read: add those addons themselves",
        }),
      ]),
    );
    const page = await openAddons();

    const lists = await within(page).findByRole('listitem', { name: 'Addon Lists' });
    expect(lists).toHaveTextContent(
      "Not for Raygun: Only lists other Stremio addons, which Raygun doesn't read",
    );
    expect(within(lists).queryByRole('switch')).toBeNull();
    const films = within(page).getByRole('listitem', { name: 'Open Films' });
    expect(within(films).getByRole('switch')).toBeInTheDocument();

    await user.click(within(lists).getByRole('button', { name: 'Remove Addon Lists' }));
    expect(commands.stremioRemove).toHaveBeenCalledWith('lists');
  });

  it('adds an addon found by its address through its own form, or says why it is not for Raygun', async () => {
    const user = userEvent.setup();
    const page = await openAddons();
    commands.sourcesDetect.mockResolvedValue(
      ok(detected(addon({ id: '', needsSetup: true, config: CONFIG }))),
    );
    commands.stremioAddConfigured.mockResolvedValue(ok(addon({ config: CONFIG })));

    await user.click(within(page).getByRole('button', { name: 'Add a source' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'films.example');
    await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Stremio addon: Open Films',
    });
    // Its settings are asked here, not on its website.
    expect(within(card).queryByText('Needs setting up on its website first')).toBeNull();
    const form = within(card).getByRole('form', { name: 'Settings of Open Films' });
    await user.type(within(form).getByRole('textbox', { name: 'API key' }), 'k');
    await user.click(within(form).getByRole('button', { name: 'Add' }));
    expect(commands.stremioAddConfigured).toHaveBeenCalledWith(
      'https://films.example/manifest.json',
      expect.arrayContaining([
        { key: 'apiKey', value: 'k' },
        { key: 'region', value: '' },
      ]),
    );
    await vi.waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Add a source' })).toBeNull(),
    );

    commands.sourcesDetect.mockResolvedValue(
      ok(
        detected(
          addon({
            id: '',
            name: 'Music Lists',
            url: 'https://music.example/manifest.json',
            notForRaygun: 'Only lists catalogs of music, which no addon here opens or plays',
          }),
        ),
      ),
    );
    await user.click(within(page).getByRole('button', { name: 'Add a source' }));
    const again = await screen.findByRole('dialog', { name: 'Add a source' });
    await user.type(within(again).getByRole('textbox', { name: 'Address' }), 'music.example');
    await user.click(within(again).getByRole('button', { name: 'Look up' }));
    const music = await within(again).findByRole('region', {
      name: 'Looks like a Stremio addon: Music Lists',
    });
    expect(music).toHaveTextContent(
      'Not for Raygun: Only lists catalogs of music, which no addon here opens or plays',
    );
    expect(within(music).queryByRole('button', { name: 'Add' })).toBeNull();
    expect(within(music).queryByRole('button', { name: 'Set up' })).toBeNull();
  });
});
