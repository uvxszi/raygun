import { commands } from '@raygun/bindings';
import type { Settings } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '../test/renderWithProviders';
import { AppearanceCard } from './AppearanceCard';
import { SAVE_DEFAULTS } from '../export/saveDefaults';
import { CACHE_DEFAULTS } from './cacheDefaults';
import { LAYOUT_DEFAULTS } from '../layout/layout';
import { LIBRARY_DISPLAY_DEFAULTS } from '../manga/libraryDisplay';
import { PLAYER_DEFAULTS } from './playerOptions';
import { SettingsSaveIssue } from './Problem';
import { useApplyTheme } from './useApplyTheme';
import { useSettings } from './useSettings';

vi.mock('@raygun/bindings', () => ({
  commands: { settingsGet: vi.fn(), settingsUpdate: vi.fn(), appInfo: vi.fn() },
}));

const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  language: 'en',
  showNsfw: false,
  explicitMusic: true,
  contentOverrides: { extensions: {}, repositories: {}, addons: {} },
  privacy: { history: true, recentlyPlayed: true },
  hidden: { artists: [], items: [] },
  lyricsCache: { forgetAfterDays: 30, keepFavorites: true, keepDownloaded: true, keepLocal: true },
  lyricsTranslation: { language: '', automatic: true, romanized: false, localModel: '' },
  accent: 'red',
  startScreen: 'home',
  navLabels: 'always',
  coverStyle: 'compact',
  unreadBadges: true,
  downloadBadges: true,
  resumeVideos: true,
  watchedPercent: 90,
  seekSeconds: 10,
  autoSubtitles: false,
  subtitleLanguage: '',
  autoplayNext: true,
  nextPromptSeconds: 10,
  videoPlayer: PLAYER_DEFAULTS,
  libraryDisplay: LIBRARY_DISPLAY_DEFAULTS,
  save: SAVE_DEFAULTS,
  cache: CACHE_DEFAULTS,
  websites: {
    blockAds: true,
    blockPopups: true,
    blocklist: 'hageziMulti',
    customListUrl: null,
    desktop: false,
  },
  layout: LAYOUT_DEFAULTS,
  accentColor: '#e5484d',
  coverSize: 0,
  mangaReader: { pagedScale: 'fitScreen', webtoonSize: 0 },
  playerView: { layout: 'auto', widthOffset: 0 },
  scrollbars: 'thin',
  motion: 'lively',
  trackTables: { tables: {} },
  panels: { railWidth: 0, sidePanelWidth: 0, playerSplit: 0, nextUpRows: 0 },
  colors: { background: '', surface: '', foreground: '', muted: '' },
  disabledSources: [],
  autoLoad: { enabled: true, cooldownMs: 1500 },
  windowFrame: 'app',
  zoom: 100,
  discovery: { region: '', tagSongs: true },
  failingSourcesLast: true,
  searchFilter: { noAdult: false, noExplicit: false },
  searchListings: { enabled: true, entries: 20, pages: 5 },
  manualBrowserChecks: false,
  userAgent: '',
  keepMusicPlaying: false,
  downloadsInBatterySaver: false,
  resumeOnStart: true,
  audioQuality: { streaming: 'high', cellular: 'normal', download: 'veryHigh' },
  rail: { pins: [], order: 'played', seeded: false },
  transcription: {
    engine: 'off',
    localModel: 'whisper-tiny',
    providerId: null,
    onlineModel: 'whisper-1',
    onlineConfirmed: false,
  },
  sound: {
    eqEnabled: false,
    eqPreset: 'flat',
    eqBands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    preamp: 0,
    bassBoost: 0,
    reverb: 'off',
    balance: 0,
    mono: false,
    normalize: false,
    normalizeTarget: -14,
    crossfadeMs: 0,
  },
  mangaUpdateHours: 12,
  novelReader: {
    textSize: 0,
    lineSpacing: 16,
    font: 'serif',
    width: 'narrow',
    paged: false,
    ttsRate: 10,
    indent: false,
  },
};

const ThemedAppearanceCard = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme);
  return (
    <>
      <AppearanceCard />
      <SettingsSaveIssue />
    </>
  );
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

    const problem = await AppearanceCardWrapper.error;
    expect(problem).toHaveTextContent("Couldn't save that change");
    await userEvent.click(within(problem).getByRole('button', { name: 'More info' }));
    expect(problem).toHaveTextContent('database is locked');
  });

  it('offers every language and saves the choice', async () => {
    vi.mocked(commands.settingsUpdate).mockResolvedValue({
      status: 'ok',
      data: { ...DEFAULT_SETTINGS, language: 'de' },
    });
    AppearanceCardWrapper.mount();

    await userEvent.click(await screen.findByRole('button', { name: /^Language/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Language' });
    expect(within(sheet).getByRole('radio', { name: /^System default/ })).not.toBeChecked();
    expect(within(sheet).getByRole('radio', { name: 'English' })).toBeChecked();
    await userEvent.click(within(sheet).getByRole('radio', { name: 'Deutsch' }));

    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'language', value: 'de' });
  });
});
