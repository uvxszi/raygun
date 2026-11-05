import type { Settings, SystemFont } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const family = (name: string, faces: SystemFont['faces'] = []): SystemFont => ({
  family: name,
  faces,
});

const startWith = (settings: Partial<Settings>) => {
  let stored: Settings = { ...DEFAULT_SETTINGS, ...settings };
  commands.settingsGet.mockImplementation(async () => ({ status: 'ok', data: stored }));
  const applied = async (changes: { key: string; value: unknown }[]) => {
    stored = changes.reduce<Settings>(
      (next, change) => ({ ...next, [change.key]: change.value }),
      stored,
    );
    return { status: 'ok' as const, data: stored };
  };
  commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) =>
    applied([change]),
  );
  commands.settingsUpdateMany.mockImplementation((changes: { key: string; value: unknown }[]) =>
    applied(changes),
  );
};

const root = () => document.documentElement.style;

describe('Fonts and text size', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    root().removeProperty('--font-ui');
    root().removeProperty('--text-scale');
  });

  it('sets the text size as an offset on the screen’s own, text alone', async () => {
    const user = userEvent.setup();
    startWith({});
    await AppShellWrapper.mount('/settings/appearance');

    const field = await screen.findByRole('spinbutton', { name: 'Text size' });
    expect(field).toHaveValue(0);
    expect(root().getPropertyValue('--text-scale')).toBe('');
    await user.clear(field);
    await user.type(field, '20{Enter}');

    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'textSize', value: 20 });
    await waitFor(() => expect(root().getPropertyValue('--text-scale')).toBe('1.2'));
    // Interface size is its own setting.
    expect(commands.settingsUpdate).not.toHaveBeenCalledWith(
      expect.objectContaining({ key: 'zoom' }),
    );
  });

  it('lists the desktop’s fonts, each in its own font, and uses the picked one by name', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.fontsList.mockResolvedValue({
      status: 'ok',
      data: [family('Inter'), family('Segoe UI Variable')],
    });
    startWith({});
    await AppShellWrapper.mount('/settings/appearance');

    await user.click(await screen.findByRole('button', { name: /Font\s*Default/ }));
    const sheet = await screen.findByRole('radiogroup', { name: 'Font' });
    expect(within(sheet).getByRole('radio', { name: 'Default' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    const inter = await within(sheet).findByRole('radio', { name: 'Inter' });
    expect(within(inter).getByText('Inter')).toHaveStyle({
      fontFamily: '"Inter", system-ui, sans-serif',
    });
    expect(within(sheet).getByRole('radio', { name: 'Segoe UI Variable' })).toBeInTheDocument();

    await user.click(inter);
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'fonts',
      value: { interface: 'Inter', reading: '' },
    });
    await waitFor(() =>
      expect(root().getPropertyValue('--font-ui')).toBe('"Inter", system-ui, sans-serif'),
    );

    await user.click(screen.getByRole('button', { name: /Font\s*Inter/ }));
    await user.click(await screen.findByRole('radio', { name: 'Default' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'fonts',
      value: { interface: '', reading: '' },
    });
    await waitFor(() => expect(root().getPropertyValue('--font-ui')).toBe(''));
  });

  it('searches a long list of fonts and looks again for new ones', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    const names = ['Arial', 'Bahnschrift', 'Calibri', 'Cambria', 'Candara', 'Consolas'];
    const listed = [...names, 'Georgia', 'Segoe UI', 'Verdana'].map((name) => family(name));
    commands.fontsList.mockResolvedValue({ status: 'ok', data: listed });
    startWith({});
    await AppShellWrapper.mount('/settings/appearance');

    await user.click(await screen.findByRole('button', { name: /Font\s*Default/ }));
    await user.type(await screen.findByRole('searchbox', { name: 'Search fonts' }), 'ca');
    const sheet = screen.getByRole('radiogroup', { name: 'Font' });
    const shown = () =>
      within(sheet)
        .getAllByRole('radio')
        .map((radio) => radio.getAttribute('aria-label'));
    expect(shown()).toEqual(['Default', 'Calibri', 'Cambria', 'Candara']);

    // A font installed while Raygun runs.
    commands.fontsList.mockResolvedValue({ status: 'ok', data: [...listed, family('Caveat')] });
    await user.click(screen.getByRole('button', { name: 'Look for new fonts' }));
    expect(commands.fontsList).toHaveBeenLastCalledWith(true);
    await waitFor(() => expect(shown()).toContain('Caveat'));
  });

  describe('on Android', () => {
    const added: { family: string; source: string; weight?: string; style?: string }[] = [];

    beforeEach(() => {
      added.length = 0;
      vi.stubGlobal(
        'FontFace',
        class {
          constructor(
            family: string,
            source: string,
            descriptors: { weight?: string; style?: string },
          ) {
            added.push({ family, source, ...descriptors });
          }
        },
      );
      Object.defineProperty(document, 'fonts', {
        configurable: true,
        value: { add: vi.fn() },
      });
    });

    afterEach(() => {
      vi.unstubAllGlobals();
      Reflect.deleteProperty(document, 'fonts');
    });

    it('loads a picked font from its files and falls back when the device lacks it', async () => {
      commands.fontsList.mockResolvedValue({
        status: 'ok',
        data: [
          family('Roboto Flex', [
            {
              url: 'http://127.0.0.1:4100/font/aa',
              weightMin: 100,
              weightMax: 1000,
              italic: false,
            },
          ]),
          family('Noto Serif', [
            { url: 'http://127.0.0.1:4100/font/bb', weightMin: 400, weightMax: 400, italic: false },
            { url: 'http://127.0.0.1:4100/font/cc', weightMin: 400, weightMax: 400, italic: true },
          ]),
        ],
      });
      startWith({ fonts: { interface: 'Noto Serif', reading: '' } });
      await AppShellWrapper.mount('/settings/appearance');

      await waitFor(() =>
        expect(root().getPropertyValue('--font-ui')).toBe('"Noto Serif", system-ui, sans-serif'),
      );
      expect(added).toEqual([
        {
          family: 'Noto Serif',
          source: 'url("http://127.0.0.1:4100/font/bb")',
          weight: '400',
          style: 'normal',
          display: 'swap',
        },
        {
          family: 'Noto Serif',
          source: 'url("http://127.0.0.1:4100/font/cc")',
          weight: '400',
          style: 'italic',
          display: 'swap',
        },
      ]);

      // The list shows the others in their own fonts, a variable one across its weights.
      const user = userEvent.setup();
      await user.click(await screen.findByRole('button', { name: /Font\s*Noto Serif/ }));
      await screen.findByRole('radio', { name: 'Roboto Flex' });
      await waitFor(() =>
        expect(added).toContainEqual(
          expect.objectContaining({ family: 'Roboto Flex', weight: '100 1000' }),
        ),
      );
    });

    it('says when a font picked elsewhere isn’t on this device, and uses the default', async () => {
      const user = userEvent.setup();
      commands.fontsList.mockResolvedValue({ status: 'ok', data: [family('Roboto Flex')] });
      startWith({ fonts: { interface: 'Gone Sans', reading: '' } });
      await AppShellWrapper.mount('/settings/appearance');

      await user.click(await screen.findByRole('button', { name: /Font\s*Gone Sans/ }));
      expect(
        await screen.findByText("Gone Sans isn't on this device; the default shows instead"),
      ).toBeInTheDocument();
      expect(root().getPropertyValue('--font-ui')).toBe('');
    });
  });

  it('lets the book reader use a device font in front of its Serif or Sans', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.fontsList.mockResolvedValue({ status: 'ok', data: [family('Literata')] });
    startWith({});
    await AppShellWrapper.mount('/settings/reader');

    await user.click(await screen.findByRole('button', { name: /Typeface\s*Serif/ }));
    const sheet = await screen.findByRole('radiogroup', { name: 'Typeface' });
    expect(
      within(sheet)
        .getAllByRole('radio')
        .map((radio) => radio.getAttribute('aria-label')),
    ).toEqual(['Serif', 'Sans serif', 'Literata']);
    await user.click(within(sheet).getByRole('radio', { name: 'Literata' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'fonts',
      value: { interface: '', reading: 'Literata' },
    });

    // Serif or Sans again: the device font goes.
    await user.click(await screen.findByRole('button', { name: /Typeface\s*Literata/ }));
    await user.click(await screen.findByRole('radio', { name: 'Sans serif' }));
    expect(commands.settingsUpdateMany).toHaveBeenLastCalledWith([
      {
        key: 'novelReader',
        value: expect.objectContaining({ font: 'sans' }),
      },
      { key: 'fonts', value: { interface: '', reading: '' } },
    ]);
  });
});
