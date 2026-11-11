import type { AudioQuality, DataSaverPrefs, NetworkPrefs, VideoResolution } from '@raygun/bindings';
import { Check, WifiOff } from 'lucide-react';
import { useId } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefSwitch } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { useIsDesktop } from '../shell/useIsDesktop';
import { SAVER_DEFAULTS, withDefaults } from './dataSaver';
import { useNetworkStatus, useOfflineMode } from './useNetworkMode';

/** The switch's knob, drawn as the settings rows draw theirs. */
const Knob: FC<{ on: boolean }> = ({ on }) => (
  <span
    aria-hidden
    className={`relative h-8 w-13 shrink-0 rounded-full border-2 transition-colors ${on ? 'bg-primary border-primary' : 'bg-surface border-muted/50'}`}
  >
    <span
      className={`absolute top-1/2 grid -translate-y-1/2 place-items-center rounded-full transition-all ${on ? 'bg-background left-[1.375rem] size-6' : 'bg-muted left-1.5 size-4'}`}
    >
      {on && <Check size={14} className="text-primary" strokeWidth={3} />}
    </span>
  </span>
);

/** Offline Mode's switch at the top of More, under incognito's. */
export const OfflineRow: FC = () => {
  const { t } = useTranslation();
  const offline = useOfflineMode();
  const titleId = useId();
  return (
    <ul className="flex flex-col">
      <li>
        <button
          type="button"
          role="switch"
          aria-checked={offline.on}
          aria-labelledby={titleId}
          disabled={offline.busy}
          onClick={() => offline.set(!offline.on)}
          className="hover:bg-surface flex w-full items-center gap-4 rounded-xl px-2 py-3 text-left disabled:opacity-60"
        >
          <WifiOff aria-hidden size={22} className="text-primary shrink-0" />
          <span className="flex min-w-0 flex-1 flex-col">
            <span id={titleId} className="font-medium">
              {t('offline.title')}
            </span>
            <span className="text-muted text-xs">{t('offline.hint')}</span>
          </span>
          <Knob on={offline.on} />
        </button>
      </li>
    </ul>
  );
};

type OfflineBadgeProps = {
  /** Shows the words beside the mark (the desktop bar). */
  labelled?: boolean;
  /** Left out on desktop, whose app bar has its own (a screen's top bar). */
  phoneOnly?: boolean;
  className?: string;
};

/** Shows that Offline Mode is on while it is; a tap turns it off. */
export const OfflineBadge: FC<OfflineBadgeProps> = ({ labelled, phoneOnly, className = '' }) => {
  const { t } = useTranslation();
  const offline = useOfflineMode();
  const desktop = useIsDesktop();
  if (!offline.on || (phoneOnly && desktop)) return null;
  return (
    <button
      type="button"
      aria-label={t('offline.turnOff')}
      data-hint={t('offline.turnOff')}
      disabled={offline.busy}
      onClick={() => offline.set(false)}
      className={`group grid h-10 shrink-0 place-items-center disabled:opacity-60 ${className}`}
    >
      <span className="bg-foreground text-background flex h-8 items-center gap-1.5 rounded-full px-2.5 text-xs font-semibold transition-opacity group-hover:opacity-85">
        <WifiOff aria-hidden size={16} />
        {labelled && <span>{t('offline.title')}</span>}
      </span>
    </button>
  );
};

/** Matches `VideoResolution` in raygun-model, from 720p down: Data Saver's caps. */
const SAVER_RESOLUTIONS: VideoResolution[] = ['720p', '480p', '360p'];
/** Data Saver's music caps. */
const SAVER_QUALITIES: AudioQuality[] = ['low', 'normal', 'high'];

/** The switches of Data Saver's parts, in the order shown. */
const SAVER_PARTS = [
  'fewerSources',
  'noPrefetch',
  'noBackgroundLookups',
  'smallerCovers',
  'noTrailers',
  'imagesWhenVisible',
] as const;

/**
 * Settings → Offline mode & data saver: Offline Mode (nothing sent over the network), then Data
 * Saver and its parts, each with its own switch.
 */
export const NetworkSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const offline = useOfflineMode();
  const status = useNetworkStatus();
  if (!data) return null;
  const network: NetworkPrefs = data.network ?? { offline: false, dataSaver: SAVER_DEFAULTS };
  const saver = withDefaults(SAVER_DEFAULTS, network.dataSaver);
  const setSaver = (change: Partial<DataSaverPrefs>) =>
    update.mutate({
      key: 'network',
      value: { ...network, offline: offline.on, dataSaver: { ...saver, ...change } },
    });
  const off = !saver.enabled;
  return (
    <>
      <PrefGroup>
        <PrefSwitch
          title={t('offline.title')}
          summary={t('offline.summary')}
          checked={offline.on}
          disabled={offline.busy}
          onChange={offline.set}
        />
      </PrefGroup>
      <PrefGroup title={t('dataSaver.title')} summary={t('dataSaver.summary')}>
        <PrefSwitch
          title={t('dataSaver.enabled')}
          summary={
            saver.enabled
              ? t(status.dataSaver ? 'dataSaver.applies' : 'dataSaver.waits')
              : undefined
          }
          checked={saver.enabled}
          onChange={(enabled) => setSaver({ enabled })}
        />
        <PrefSwitch
          title={t('dataSaver.mobileDataOnly')}
          checked={saver.mobileDataOnly}
          disabled={off}
          onChange={(mobileDataOnly) => setSaver({ mobileDataOnly })}
        />
        <PrefSwitch
          title={t('dataSaver.limitVideo')}
          checked={saver.limitVideo}
          disabled={off}
          onChange={(limitVideo) => setSaver({ limitVideo })}
        />
        {saver.limitVideo && (
          <PrefChoice<VideoResolution>
            title={t('dataSaver.videoResolution')}
            value={saver.videoResolution}
            options={SAVER_RESOLUTIONS.map((resolution) => ({
              value: resolution,
              label: resolution,
            }))}
            disabled={off}
            onChange={(videoResolution) => setSaver({ videoResolution })}
          />
        )}
        <PrefSwitch
          title={t('dataSaver.limitMusic')}
          checked={saver.limitMusic}
          disabled={off}
          onChange={(limitMusic) => setSaver({ limitMusic })}
        />
        {saver.limitMusic && (
          <PrefChoice<AudioQuality>
            title={t('dataSaver.musicQuality')}
            value={saver.musicQuality}
            options={SAVER_QUALITIES.map((level) => ({
              value: level,
              label: t(`settings.audioQuality.levels.${level}`),
              hint: t(`settings.audioQuality.hints.${level}`),
            }))}
            disabled={off}
            onChange={(musicQuality) => setSaver({ musicQuality })}
          />
        )}
        {SAVER_PARTS.map((part) => (
          <PrefSwitch
            key={part}
            title={t(`dataSaver.${part}`)}
            checked={saver[part]}
            disabled={off}
            onChange={(on) => {
              const change: Partial<DataSaverPrefs> = {};
              change[part] = on;
              setSaver(change);
            }}
          />
        ))}
      </PrefGroup>
    </>
  );
};
