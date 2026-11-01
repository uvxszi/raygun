import type { CoverStyle, NovelFont, NovelReaderPrefs, NovelWidth } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { Bot, Package, Puzzle, Smartphone, Tag } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useAiProviders } from '../ai/useAi';
import { useSyncStatus } from '../devices/useDeviceSync';
import { useExtensions } from '../extensions/useExtensions';
import { LANGUAGES, nativeName } from '../i18n/languages';
import { LibraryFolders } from '../library/LibraryFolders';
import { useLibraryStatus } from '../library/useLibrary';
import { AppearanceCard } from '../settings/AppearanceCard';
import { BackupCard } from '../settings/BackupCard';
import { McpCard } from '../settings/McpCard';
import {
  PrefBlock,
  PrefChoice,
  PrefGroup,
  PrefLink,
  PrefSlider,
  PrefSwitch,
} from '../settings/prefs';
import { RemoteCard } from '../settings/RemoteCard';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { TopBar } from '../shell/TopBar';
import type { SettingsSection } from '../settings/sections';

/** Matches `MANGA_UPDATE_HOURS`, `VIDEO_QUALITIES` and `SEEK_SECONDS` in raygun-model. */
const UPDATE_HOURS = [0, 6, 12, 24, 48, 168];
const QUALITIES = [1080, 720, 480, 360];
const SEEK_SECONDS = [5, 10, 15, 30, 60];
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
  return (
    <>
      <PrefGroup title={t('settings.display')}>
        <PrefChoice<number>
          title={t('settings.gridColumns')}
          value={data.gridColumns}
          disabled={busy}
          options={[0, 2, 3, 4, 5, 6].map((value) => ({
            value,
            label: value === 0 ? t('settings.gridColumnsAuto') : String(value),
          }))}
          onChange={(value) => update.mutate({ key: 'gridColumns', value })}
        />
        <PrefChoice<CoverStyle>
          title={t('settings.coverStyle')}
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
  return (
    <>
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
        <PrefChoice<number>
          title={t('settings.seekSeconds')}
          summary={t('settings.seekSecondsHint')}
          value={data.seekSeconds}
          disabled={busy}
          options={SEEK_SECONDS.map((seconds) => ({
            value: seconds,
            label: t('settings.seconds', { count: seconds }),
          }))}
          onChange={(value) => update.mutate({ key: 'seekSeconds', value })}
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
      <PrefGroup title={t('settings.subtitles')}>
        <PrefSwitch
          title={t('settings.autoSubtitles')}
          summary={t('settings.autoSubtitlesHint')}
          checked={data.autoSubtitles}
          disabled={busy}
          onChange={(value) => update.mutate({ key: 'autoSubtitles', value })}
        />
        <PrefChoice<string>
          title={t('settings.subtitleLanguage')}
          summary={t('settings.subtitleLanguageHint')}
          value={data.subtitleLanguage}
          disabled={busy}
          options={[
            { value: '', label: t('settings.subtitleLanguageApp') },
            ...LANGUAGES.map((tag) => ({ value: tag, label: nativeName(tag) })),
          ]}
          onChange={(value) => update.mutate({ key: 'subtitleLanguage', value })}
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

const SECTIONS: Record<SettingsSection, FC> = {
  appearance: AppearanceCard,
  library: LibrarySettings,
  reader: ReaderSettings,
  player: PlayerSettings,
  browse: BrowseSettings,
  data: () => (
    <PrefGroup>
      <PrefBlock>
        <BackupCard />
      </PrefBlock>
    </PrefGroup>
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
