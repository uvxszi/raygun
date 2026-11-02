import type {
  CoverStyle,
  LibraryDisplayMode,
  LibraryTabs,
  NovelFont,
  NovelReaderPrefs,
  NovelWidth,
} from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { Bot, LayoutList, Package, Puzzle, Smartphone, Tag } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useAiProviders } from '../ai/useAi';
import { useSyncStatus } from '../devices/useDeviceSync';
import { useExtensions } from '../extensions/useExtensions';
import { LibraryFolders } from '../library/LibraryFolders';
import {
  DISPLAY_MODES,
  COVER_WIDTHS,
  GRID_COLUMNS,
  TAB_MODES,
  withLibraryDisplay,
} from '../manga/libraryDisplay';
import type { LibraryDisplay } from '../manga/libraryDisplay';
import { useLibraryStatus } from '../library/useLibrary';
import { ReaderDisplaySettings } from '../manga/ReaderDisplaySettings';
import { PlayerViewSettings } from '../player/PlayerViewSettings';
import { AppearanceCard } from '../settings/AppearanceCard';
import { BackupCard } from '../settings/BackupCard';
import { DownloadSettings } from '../settings/DownloadSettings';
import { McpCard } from '../settings/McpCard';
import { PLAYER_PAGES, withDefaults } from '../settings/playerOptions';
import type { PlayerPrefs } from '../settings/playerOptions';
import {
  PrefBlock,
  PrefChoice,
  PrefGroup,
  PrefLink,
  PrefSlider,
  PrefSwitch,
} from '../settings/prefs';
import { RemoteCard } from '../settings/RemoteCard';
import { StorageSettings } from '../settings/StorageSettings';
import { WebsiteSettings } from '../websites/WebsiteSettings';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { TopBar } from '../shell/TopBar';
import type { SettingsSection } from '../settings/sections';

/** Matches `MANGA_UPDATE_HOURS` and `VIDEO_QUALITIES` in raygun-model. */
const UPDATE_HOURS = [0, 6, 12, 24, 48, 168];
const QUALITIES = [1080, 720, 480, 360];
/** Matches `NEXT_PROMPT_SECONDS` in raygun-model (0: at the end). */
const NEXT_PROMPT_SECONDS = [0, 5, 10, 15, 30];

const useSetting = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  return { data: settings.data, update, busy: update.isPending };
};

const SaveError: FC = () => {
  const { t } = useTranslation();
  const update = useUpdateSetting();
  return update.isError ? (
    <p role="alert" className="text-primary px-1 text-sm">
      {t('settings.saveFailed', { message: update.error.message })}
    </p>
  ) : null;
};

const LibrarySettings: FC = () => {
  const { t } = useTranslation();
  const { data, update, busy } = useSetting();
  const status = useLibraryStatus();
  if (!data) return null;
  const libraryDisplay = withLibraryDisplay(data.libraryDisplay);
  const setLibraryDisplay = (change: Partial<LibraryDisplay>) =>
    update.mutate({ key: 'libraryDisplay', value: { ...libraryDisplay, ...change } });
  return (
    <>
      <PrefGroup title={t('settings.libraryDisplay')}>
        <PrefChoice<LibraryDisplayMode>
          title={t('library.display.mode')}
          value={libraryDisplay.mode}
          disabled={busy}
          options={DISPLAY_MODES.map((value) => ({
            value,
            label: t(`library.display.modes.${value}`),
          }))}
          onChange={(mode) => setLibraryDisplay({ mode })}
        />
        <PrefChoice<number>
          title={t('settings.gridColumns')}
          value={data.gridColumns}
          disabled={busy}
          options={GRID_COLUMNS.map((value) => ({
            value,
            label: value === 0 ? t('settings.gridColumnsAuto') : String(value),
          }))}
          onChange={(value) => update.mutate({ key: 'gridColumns', value })}
        />
        <PrefChoice<number>
          title={t('library.display.columnsLandscape')}
          value={libraryDisplay.columnsLandscape}
          disabled={busy}
          options={GRID_COLUMNS.map((value) => ({
            value,
            label: value === 0 ? t('settings.gridColumnsAuto') : String(value),
          }))}
          onChange={(columnsLandscape) => setLibraryDisplay({ columnsLandscape })}
        />
        <PrefChoice<number>
          title={t('settings.coverWidth')}
          summary={t('settings.coverWidthHint')}
          value={data.coverWidth ?? 0}
          disabled={busy}
          options={COVER_WIDTHS.map((value) => ({
            value,
            label:
              value === 0 ? t('settings.coverWidthAuto') : t('settings.coverWidthPx', { n: value }),
          }))}
          onChange={(value) => update.mutate({ key: 'coverWidth', value })}
        />
        <PrefChoice<LibraryTabs>
          title={t('library.display.tabs')}
          value={libraryDisplay.tabs}
          disabled={busy}
          options={TAB_MODES.map((value) => ({
            value,
            label: t(`library.display.tabModes.${value}`),
          }))}
          onChange={(tabs) => setLibraryDisplay({ tabs })}
        />
        <PrefSwitch
          title={t('library.display.showCounts')}
          checked={libraryDisplay.showCounts}
          disabled={busy}
          onChange={(showCounts) => setLibraryDisplay({ showCounts })}
        />
        <PrefSwitch
          title={t('library.display.perCategory')}
          checked={libraryDisplay.perCategory}
          disabled={busy}
          onChange={(perCategory) => setLibraryDisplay({ perCategory })}
        />
        <PrefSwitch
          title={t('library.display.continueReading')}
          checked={libraryDisplay.continueButton}
          disabled={busy}
          onChange={(continueButton) => setLibraryDisplay({ continueButton })}
        />
        <PrefSwitch
          title={t('library.display.pinsOnTop')}
          checked={libraryDisplay.pinsOnTop}
          disabled={busy}
          onChange={(pinsOnTop) => setLibraryDisplay({ pinsOnTop })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.display')}>
        <PrefChoice<CoverStyle>
          title={t('settings.browseCovers')}
          value={data.coverStyle}
          disabled={busy}
          options={[
            {
              value: 'compact',
              label: t('settings.coverCompact'),
              hint: t('settings.coverCompactHint'),
            },
            {
              value: 'comfortable',
              label: t('settings.coverComfortable'),
              hint: t('settings.coverComfortableHint'),
            },
          ]}
          onChange={(value) => update.mutate({ key: 'coverStyle', value })}
        />
        <PrefSwitch
          title={t('settings.unreadBadges')}
          summary={t('settings.unreadBadgesHint')}
          checked={data.unreadBadges}
          disabled={busy}
          onChange={(value) => update.mutate({ key: 'unreadBadges', value })}
        />
        <PrefSwitch
          title={t('settings.downloadBadges')}
          summary={t('settings.downloadBadgesHint')}
          checked={data.downloadBadges}
          disabled={busy}
          onChange={(value) => update.mutate({ key: 'downloadBadges', value })}
        />
        <PrefSwitch
          title={t('library.display.localBadge')}
          checked={libraryDisplay.localBadge}
          disabled={busy}
          onChange={(localBadge) => setLibraryDisplay({ localBadge })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.libraryUpdates')}>
        <PrefChoice<number>
          title={t('settings.mangaUpdates')}
          summary={t('settings.mangaUpdatesHint')}
          value={data.mangaUpdateHours}
          disabled={busy}
          options={UPDATE_HOURS.map((hours) => ({
            value: hours,
            label: t(`settings.updateEvery.${hours}`),
          }))}
          onChange={(value) => update.mutate({ key: 'mangaUpdateHours', value })}
        />
      </PrefGroup>
      <PrefGroup title={t('library.categories.title')}>
        <PrefLink
          icon={Tag}
          to="/library/categories"
          search={{ medium: 'manga' }}
          title={t('library.categories.mangaTitle')}
        />
        <PrefLink
          icon={Tag}
          to="/library/categories"
          search={{ medium: 'novel' }}
          title={t('library.categories.novelTitle')}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.musicFolders')}>
        <PrefBlock>
          <p className="text-muted mb-3 text-sm">{t('settings.musicFoldersHint')}</p>
          <LibraryFolders roots={status.data?.roots ?? []} />
        </PrefBlock>
      </PrefGroup>
    </>
  );
};

const ReaderSettings: FC = () => {
  const { t } = useTranslation();
  const { data, update, busy } = useSetting();
  if (!data) return null;
  const prefs = data.novelReader;
  const set = (patch: Partial<NovelReaderPrefs>) =>
    update.mutate({ key: 'novelReader', value: { ...prefs, ...patch } });
  return (
    <>
      <ReaderDisplaySettings title={t('reader.display.mangaTitle')} />
      <PrefGroup title={t('settings.novelReader')}>
        <PrefSlider
          title={t('settings.fontSize')}
          value={prefs.fontSize}
          min={14}
          max={30}
          step={1}
          format={(value) => `${value} px`}
          disabled={busy}
          onChange={(fontSize) => set({ fontSize })}
        />
        <PrefSlider
          title={t('settings.lineSpacing')}
          value={prefs.lineSpacing}
          min={12}
          max={22}
          step={1}
          format={(value) => (value / 10).toFixed(1)}
          disabled={busy}
          onChange={(lineSpacing) => set({ lineSpacing })}
        />
        <PrefChoice<NovelFont>
          title={t('settings.font')}
          value={prefs.font}
          disabled={busy}
          options={[
            { value: 'serif', label: t('settings.fontSerif') },
            { value: 'sans', label: t('settings.fontSans') },
          ]}
          onChange={(font) => set({ font })}
        />
        <PrefChoice<NovelWidth>
          title={t('settings.pageWidth')}
          value={prefs.width}
          disabled={busy}
          options={[
            {
              value: 'narrow',
              label: t('settings.widthNarrow'),
              hint: t('settings.widthNarrowHint'),
            },
            { value: 'wide', label: t('settings.widthWide'), hint: t('settings.widthWideHint') },
          ]}
          onChange={(width) => set({ width })}
        />
        <PrefSwitch
          title={t('settings.paged')}
          summary={t('settings.pagedHint')}
          checked={prefs.paged}
          disabled={busy}
          onChange={(paged) => set({ paged })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.readAloud')}>
        <PrefSlider
          title={t('settings.speechRate')}
          summary={t('settings.speechRateHint')}
          value={prefs.ttsRate ?? 10}
          min={5}
          max={20}
          step={1}
          format={(value) => `${(value / 10).toFixed(1)}×`}
          disabled={busy}
          onChange={(ttsRate) => set({ ttsRate })}
        />
      </PrefGroup>
    </>
  );
};

const PlayerSettings: FC = () => {
  const { t } = useTranslation();
  const { data, update, busy } = useSetting();
  if (!data) return null;
  const prefs = withDefaults(data.videoPlayer);
  const setPlayer = (change: Partial<PlayerPrefs>) =>
    update.mutate({ key: 'videoPlayer', value: { ...prefs, ...change } });
  return (
    <>
      <PrefGroup>
        {PLAYER_PAGES.map((page) => (
          <PrefLink
            key={page.id}
            icon={page.icon}
            title={t(`settings.player.section.${page.id}.title`)}
            summary={t(`settings.player.section.${page.id}.summary`)}
            to={`/settings/player/${page.id}`}
          />
        ))}
      </PrefGroup>
      <PlayerViewSettings title={t('player.view.musicTitle')} />
      <PrefGroup title={t('settings.videoPlayback')}>
        <PrefChoice<number | null>
          title={t('settings.videoQuality')}
          summary={t('settings.videoQualityHint')}
          value={data.preferredVideoQuality}
          disabled={busy}
          options={[
            { value: null, label: t('settings.videoQualityHighest') },
            ...QUALITIES.map((height) => ({
              value: height,
              label: t('settings.videoQualityUpTo', { height }),
            })),
          ]}
          onChange={(value) => update.mutate({ key: 'preferredVideoQuality', value })}
        />
        <PrefSwitch
          title={t('settings.resumeVideos')}
          summary={t('settings.resumeVideosHint')}
          checked={data.resumeVideos}
          disabled={busy}
          onChange={(value) => update.mutate({ key: 'resumeVideos', value })}
        />
        <PrefSlider
          title={t('settings.watchedPercent')}
          summary={t('settings.watchedPercentHint')}
          value={data.watchedPercent}
          min={50}
          max={100}
          step={5}
          format={(value) => `${value}%`}
          disabled={busy}
          onChange={(value) => update.mutate({ key: 'watchedPercent', value })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.episodes')}>
        <PrefSwitch
          title={t('settings.autoplayNext')}
          summary={t('settings.autoplayNextHint')}
          checked={data.autoplayNext}
          disabled={busy}
          onChange={(value) => update.mutate({ key: 'autoplayNext', value })}
        />
        <PrefChoice<number>
          title={t('settings.nextPrompt')}
          summary={t('settings.nextPromptHint')}
          value={data.nextPromptSeconds}
          disabled={busy}
          options={NEXT_PROMPT_SECONDS.map((seconds) => ({
            value: seconds,
            label:
              seconds === 0
                ? t('settings.nextPromptAtEnd')
                : t('settings.nextPromptBefore', { count: seconds }),
          }))}
          onChange={(value) => update.mutate({ key: 'nextPromptSeconds', value })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.sourcesList')}>
        <PrefSwitch
          title={t('settings.player.showFailedSources')}
          summary={t('settings.player.showFailedSourcesHint')}
          checked={prefs.showFailedSources}
          disabled={busy}
          onChange={(showFailedSources) => setPlayer({ showFailedSources })}
        />
        <PrefSwitch
          title={t('settings.player.showEmptySources')}
          checked={prefs.showEmptySources}
          disabled={busy}
          onChange={(showEmptySources) => setPlayer({ showEmptySources })}
        />
      </PrefGroup>
    </>
  );
};

const BrowseSettings: FC = () => {
  const { t } = useTranslation();
  const { data, update, busy } = useSetting();
  const queryClient = useQueryClient();
  const extensions = useExtensions();
  const failed = extensions.data?.filter((extension) => extension.status === 'failed').length ?? 0;
  if (!data) return null;
  return (
    <>
      <PrefGroup title={t('settings.content')}>
        <PrefSwitch
          title={t('settings.showNsfw')}
          summary={t('settings.showNsfwHint')}
          checked={data.showNsfw}
          disabled={busy}
          onChange={(value) =>
            update.mutate(
              { key: 'showNsfw', value },
              // Sources, listings and the library all depend on it.
              { onSuccess: () => void queryClient.invalidateQueries() },
            )
          }
        />
      </PrefGroup>
      <WebsiteSettings />
      <PrefGroup title={t('settings.sources')}>
        <PrefLink
          icon={Puzzle}
          to="/browse"
          search={{ tab: 'extensions' }}
          title={t('settings.extensions')}
          summary={
            <span data-testid="extensions-card">
              {extensions.data
                ? extensions.data.map((extension) => extension.name).join(', ') ||
                  t('extensions.noneInstalled')
                : ''}
              {failed > 0 && (
                <span role="alert" className="text-primary block">
                  {t('settings.extensionsFailed', { count: failed })}
                </span>
              )}
            </span>
          }
        />
        <PrefLink
          icon={Package}
          to="/extensions/repos"
          title={t('browse.repos')}
          summary={t('more.reposHint')}
        />
        <PrefLink
          icon={Puzzle}
          to="/watch/addons"
          title={t('watch.addons')}
          summary={t('more.addonsHint')}
        />
      </PrefGroup>
    </>
  );
};

const ConnectionSettings: FC = () => {
  const { t } = useTranslation();
  const providers = useAiProviders();
  const models = useAiModels();
  const installed = models.data?.filter((model) => model.installed).length ?? 0;
  const sync = useSyncStatus().data;
  return (
    <>
      <PrefGroup title={t('settings.otherDevices')}>
        <PrefLink
          icon={Smartphone}
          to="/settings/devices"
          title={t('more.devices')}
          summary={
            <span data-testid="devices-card">
              {!sync
                ? t('more.devicesHint')
                : !sync.enabled
                  ? t('devices.off')
                  : sync.peers.length === 0
                    ? t('devices.noPeersShort')
                    : sync.peers.map((peer) => peer.name).join(', ')}
            </span>
          }
        />
        <PrefBlock>
          <RemoteCard />
        </PrefBlock>
      </PrefGroup>
      <PrefGroup title={t('settings.assistants')}>
        <PrefLink
          icon={Bot}
          to="/settings/ai"
          title={t('more.ai')}
          summary={
            <span data-testid="ai-card">
              {t('ai.summary', { count: providers.data?.length ?? 0, models: installed })}
            </span>
          }
        />
        <PrefBlock>
          <McpCard />
        </PrefBlock>
      </PrefGroup>
    </>
  );
};

/** Appearance, and the way to arrange tabs and screens (M46.5). */
const AppearanceSettings: FC = () => {
  const { t } = useTranslation();
  return (
    <>
      <AppearanceCard />
      <PrefGroup title={t('layout.title')}>
        <PrefLink
          title={t('layout.title')}
          summary={t('layout.linkHint')}
          to="/settings/layout"
          icon={LayoutList}
        />
      </PrefGroup>
    </>
  );
};

const SECTIONS: Record<SettingsSection, FC> = {
  appearance: AppearanceSettings,
  library: LibrarySettings,
  reader: ReaderSettings,
  player: PlayerSettings,
  downloads: DownloadSettings,
  browse: BrowseSettings,
  data: () => (
    <>
      <PrefGroup>
        <PrefBlock>
          <BackupCard />
        </PrefBlock>
      </PrefGroup>
      <StorageSettings />
    </>
  ),
  connections: ConnectionSettings,
};

/** One settings category (`/settings/<section>`). */
export const SettingsSectionPage: FC = () => {
  const { t } = useTranslation();
  const section = useRouterState({
    select: (state) => state.location.pathname.split('/')[2] as SettingsSection,
  });
  const Section = SECTIONS[section];
  return (
    <section data-testid={`settings-${section}-page`} className="flex max-w-2xl flex-col">
      <TopBar back fallback="/settings" title={t(`settings.section.${section}.title`)} />
      {Section && <Section />}
      <SaveError />
    </section>
  );
};
