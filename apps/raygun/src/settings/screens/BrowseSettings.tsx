import { useQueryClient } from '@tanstack/react-query';
import { EyeOff, Package, Puzzle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useExtensions } from '../../extensions/useExtensions';
import { useLayout } from '../../layout/useLayout';
import { autoRegion } from '../../music/useDiscovery';
import { BROWSE_TAB_LABEL } from '../../pages/browseTabs';
import type { BrowseSection } from '../../pages/BrowsePage';
import { WebsiteSettings } from '../../websites/WebsiteSettings';
import { ContentMarks } from '../ContentMarks';
import { ManualBrowserChecks } from '../ManualBrowserChecks';
import { RunSourceCode } from '../RunSourceCode';
import { PrefChoice, PrefGroup, PrefLink, PrefSlider, PrefSwitch, PrefText } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** A User-Agent must be printable ASCII (what an HTTP header can carry), up to 512 characters. */
const isHeaderText = (text: string) =>
  text.length <= 512 &&
  [...text].every((char) => char.charCodeAt(0) >= 32 && char.charCodeAt(0) < 127);

/** Countries offered for charts (M46.9); Apple's charts cover these. */
const CHART_REGIONS = [
  'us',
  'gb',
  'ca',
  'au',
  'ie',
  'nz',
  'de',
  'fr',
  'es',
  'it',
  'nl',
  'be',
  'se',
  'no',
  'dk',
  'fi',
  'pl',
  'pt',
  'at',
  'ch',
  'br',
  'mx',
  'ar',
  'cl',
  'co',
  'jp',
  'kr',
  'tw',
  'hk',
  'sg',
  'in',
  'id',
  'ph',
  'th',
  'vn',
  'my',
  'za',
  'ng',
  'eg',
  'sa',
  'ae',
  'tr',
  'ua',
  'il',
];

const countryName = (code: string) => {
  try {
    return (
      new Intl.DisplayNames([navigator.language], { type: 'region' }).of(code.toUpperCase()) ?? code
    );
  } catch {
    return code.toUpperCase();
  }
};

/** Settings → Browse: what's shown, sources and how they're asked, charts, and websites. */
export const BrowseSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const layout = useLayout();
  const queryClient = useQueryClient();
  const extensions = useExtensions();
  const failed = extensions.data?.filter((extension) => extension.status === 'failed').length ?? 0;
  if (!data) return null;
  // Sources, listings, searches and the library all depend on these.
  const refreshAll = { onSuccess: () => void queryClient.invalidateQueries() };
  const discovery = {
    region: data.discovery?.region ?? '',
    tagSongs: data.discovery?.tagSongs ?? true,
  };
  const autoLoad = {
    enabled: data.autoLoad?.enabled ?? true,
    cooldownMs: data.autoLoad?.cooldownMs ?? 1500,
  };
  // Sources whose search fails or finds nothing are looked through instead (M46.49).
  const listings = {
    enabled: data.searchListings?.enabled ?? true,
    entries: data.searchListings?.entries ?? 20,
    pages: data.searchListings?.pages ?? 5,
  };
  const updateListings = (change: Partial<typeof listings>) =>
    update.mutate({ key: 'searchListings', value: { ...listings, ...change } });

  return (
    <>
      <PrefGroup title={t('settings.content')}>
        <PrefChoice
          title={t('settings.browseStartTab')}
          value={data.browseStartTab ?? 'discover'}
          options={layout.browseTabs
            .filter((item) => item.visible || item.id === data.browseStartTab)
            .flatMap((item) =>
              item.id in BROWSE_TAB_LABEL
                ? [{ value: item.id, label: t(BROWSE_TAB_LABEL[item.id as BrowseSection]) }]
                : [],
            )}
          onChange={(value) => update.mutate({ key: 'browseStartTab', value })}
        />
        <PrefSwitch
          title={t('settings.showNsfw')}
          checked={data.showNsfw}
          onChange={(value) => update.mutate({ key: 'showNsfw', value }, refreshAll)}
        />
        <PrefSwitch
          title={t('settings.explicitMusic')}
          checked={data.explicitMusic}
          onChange={(value) => update.mutate({ key: 'explicitMusic', value }, refreshAll)}
        />
        <ContentMarks />
        {/* Don't show this artist and Not interested, to show again. */}
        <PrefLink
          icon={EyeOff}
          to="/settings/hidden"
          title={t('hidden.title')}
          summary={
            data.hidden.artists.length + data.hidden.items.length === 0
              ? t('hidden.none')
              : [
                  t('hidden.summaryArtists', { count: data.hidden.artists.length }),
                  t('hidden.summaryItems', { count: data.hidden.items.length }),
                ].join(' · ')
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
              <span className="block truncate">
                {extensions.data
                  ? extensions.data.map((extension) => extension.name).join(', ') ||
                    t('extensions.noneInstalled')
                  : ''}
              </span>
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
          to="/sources"
          title={t('sources.title')}
          summary={t('sources.page.summary')}
        />
        <PrefSwitch
          title={t('sources.failingLast')}
          checked={data.failingSourcesLast ?? true}
          onChange={(value) => update.mutate({ key: 'failingSourcesLast', value })}
        />
        <PrefSwitch
          title={t('settings.searchListings.title')}
          summary={t('settings.searchListings.hint')}
          checked={listings.enabled}
          onChange={(enabled) => updateListings({ enabled })}
        />
        <PrefSlider
          title={t('settings.searchListings.entries')}
          summary={t('settings.searchListings.entriesHint')}
          value={listings.entries}
          min={5}
          max={100}
          step={5}
          format={String}
          disabled={!listings.enabled}
          onChange={(entries) => updateListings({ entries })}
        />
        <PrefSlider
          title={t('settings.searchListings.pages')}
          summary={t('settings.searchListings.pagesHint')}
          value={listings.pages}
          min={1}
          max={20}
          step={1}
          format={String}
          disabled={!listings.enabled}
          onChange={(pages) => updateListings({ pages })}
        />
        <PrefSwitch
          title={t('settings.autoLoad.title')}
          checked={autoLoad.enabled}
          onChange={(enabled) =>
            update.mutate({ key: 'autoLoad', value: { ...autoLoad, enabled } })
          }
        />
        <PrefSlider
          title={t('settings.autoLoad.cooldown')}
          value={autoLoad.cooldownMs}
          min={0}
          max={10_000}
          step={500}
          format={(value) => t('settings.autoLoad.seconds', { n: value / 1000 })}
          disabled={!autoLoad.enabled}
          onChange={(cooldownMs) =>
            update.mutate({ key: 'autoLoad', value: { ...autoLoad, cooldownMs } })
          }
        />
        <PrefText
          title={t('sources.userAgent')}
          summary={t('sources.userAgentHint')}
          value={data.userAgent ?? ''}
          placeholder={t('sources.userAgentDefault')}
          check={(text) => (isHeaderText(text) ? null : t('sources.userAgentInvalid'))}
          onChange={(value) => update.mutate({ key: 'userAgent', value })}
        />
        <ManualBrowserChecks />
        <RunSourceCode />
      </PrefGroup>
      <PrefGroup title={t('discover.charts')}>
        <PrefChoice<string>
          title={t('discover.region')}
          value={discovery.region}
          options={[
            { value: '', label: t('discover.regionAuto', { region: countryName(autoRegion()) }) },
            ...CHART_REGIONS.map((code) => ({ value: code, label: countryName(code) })),
          ]}
          onChange={(region) =>
            update.mutate({ key: 'discovery', value: { ...discovery, region } })
          }
        />
        <PrefSwitch
          title={t('discover.tagSongsTitle')}
          checked={discovery.tagSongs}
          onChange={(tagSongs) =>
            update.mutate({ key: 'discovery', value: { ...discovery, tagSongs } })
          }
        />
      </PrefGroup>
      <WebsiteSettings />
    </>
  );
};
