import { commands } from '@raygun/bindings';
import type { Settings } from '@raygun/bindings';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '../test/renderWithProviders';
import { AppearanceCard } from './AppearanceCard';
import { useApplyTheme } from './useApplyTheme';
import { useSettings } from './useSettings';

vi.mock('@raygun/bindings', () => ({
  commands: { settingsGet: vi.fn(), settingsUpdate: vi.fn() },
}));

const DEFAULT_SETTINGS: Settings = { theme: 'system', language: 'en', mangaUpdateHours: 12 };

const ThemedAppearanceCard = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme);
  return <AppearanceCard />;
};

const AppearanceCardWrapper = {
  mount() {
    renderWithProviders(<ThemedAppearanceCard />);
  },
  async themeOption(label: string) {
    return screen.findByRole('radio', { name: label });
  },
  async chooseTheme(label: string) {
    await userEvent.click(await this.themeOption(label));
  },
  get error() {
    return screen.findByRole('alert');
  },
};

describe('AppearanceCard', () => {
  beforeEach(() => {
    delete document.documentElement.dataset.theme;
    vi.mocked(commands.settingsGet).mockResolvedValue({ status: 'ok', data: DEFAULT_SETTINGS });
  });

  it('marks the stored theme as selected', async () => {
    AppearanceCardWrapper.mount();

    expect(await AppearanceCardWrapper.themeOption('System')).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(await AppearanceCardWrapper.themeOption('Light')).toHaveAttribute(
      'aria-checked',
      'false',
    );
  });

  it('saves the chosen theme and applies it to the page', async () => {
    vi.mocked(commands.settingsUpdate).mockResolvedValue({
      status: 'ok',
      data: { ...DEFAULT_SETTINGS, theme: 'light' },
    });
    AppearanceCardWrapper.mount();

    await AppearanceCardWrapper.chooseTheme('Light');

    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'theme', value: 'light' });
    expect(await AppearanceCardWrapper.themeOption('Light')).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('follows the system theme by removing the override', async () => {
    document.documentElement.dataset.theme = 'dark';
    AppearanceCardWrapper.mount();

    await AppearanceCardWrapper.themeOption('System');

    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('reports a failed save', async () => {
    vi.mocked(commands.settingsUpdate).mockResolvedValue({
      status: 'error',
      error: 'database is locked',
    });
    AppearanceCardWrapper.mount();

    await AppearanceCardWrapper.chooseTheme('AMOLED');

    expect(await AppearanceCardWrapper.error).toHaveTextContent('database is locked');
  });
});
