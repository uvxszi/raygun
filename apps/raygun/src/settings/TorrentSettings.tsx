import type { TorrentPrefs } from '@raygun/bindings';
import { Share2, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { PrefChoice, PrefGroup, PrefSwitch, PrefText } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

/** Matches `TORRENT_CACHE_GB` in raygun-model. */
const TORRENT_CACHE_GB = [1, 2, 4, 8, 16, 32, 64];

/** Matches `TorrentPrefs::default()` in raygun-model. */
const TORRENT_DEFAULTS: Required<TorrentPrefs> = {
  enabled: false,
  wifiOnly: true,
  cacheGb: 4,
  seed: false,
  port: 0,
  dht: true,
};

const NOTES = ['share', 'responsible', 'data', 'sources'] as const;

/** A port as typed: empty or 0 for any free one, else 1024 to 65535. */
const portProblem = (text: string): boolean => {
  if (text === '' || text === '0') return false;
  if (!/^\d{1,5}$/.test(text)) return true;
  const port = Number(text);
  return port < 1024 || port > 65535;
};

/**
 * Settings → Video player → Torrents (`Settings::torrents`, this device only): videos sources
 * give as torrents play in the player. Off by default, turned on only after the notes are read
 * (torrents share the video's pieces with other people while it plays); turning it off needs no
 * question.
 */
export const TorrentSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const [asking, setAsking] = useState(false);
  if (!data) return null;
  const prefs: Required<TorrentPrefs> = { ...TORRENT_DEFAULTS, ...data.torrents };
  const set = (change: Partial<TorrentPrefs>) =>
    update.mutate({ key: 'torrents', value: { ...prefs, ...change } });
  const off = !prefs.enabled;
  return (
    <PrefGroup id="torrents" title={t('settings.torrents.title')}>
      <PrefSwitch
        title={t('settings.torrents.enabled')}
        summary={t('settings.torrents.enabledHint')}
        checked={prefs.enabled}
        onChange={(value) => (value ? setAsking(true) : set({ enabled: false }))}
      />
      <PrefSwitch
        title={t('settings.torrents.wifiOnly')}
        summary={t('settings.torrents.wifiOnlyHint')}
        checked={prefs.wifiOnly}
        disabled={off}
        onChange={(wifiOnly) => set({ wifiOnly })}
      />
      <PrefChoice<number>
        title={t('settings.torrents.cache')}
        summary={t('settings.torrents.cacheHint')}
        value={prefs.cacheGb}
        disabled={off}
        options={TORRENT_CACHE_GB.map((value) => ({
          value,
          label: t('settings.torrents.gigabytes', { count: value }),
        }))}
        onChange={(cacheGb) => set({ cacheGb })}
      />
      <PrefSwitch
        title={t('settings.torrents.seed')}
        summary={t('settings.torrents.seedHint')}
        checked={prefs.seed}
        disabled={off}
        onChange={(seed) => set({ seed })}
      />
      <PrefSwitch
        title={t('settings.torrents.dht')}
        summary={t('settings.torrents.dhtHint')}
        checked={prefs.dht}
        disabled={off}
        onChange={(dht) => set({ dht })}
      />
      <PrefText
        title={t('settings.torrents.port')}
        summary={t('settings.torrents.portHint')}
        value={prefs.port === 0 ? '' : String(prefs.port)}
        placeholder={t('settings.torrents.anyPort')}
        disabled={off}
        check={(text) => (portProblem(text) ? t('settings.torrents.portProblem') : null)}
        onChange={(text) => set({ port: text === '' ? 0 : Number(text) })}
      />
      {asking && (
        <Sheet title={t('settings.torrents.confirmTitle')} onClose={() => setAsking(false)}>
          <div data-testid="torrent-notes" className="flex flex-col gap-4 pb-1">
            <ul className="flex flex-col gap-2 text-sm">
              {NOTES.map((note) => (
                <li key={note} className="flex gap-2">
                  {note === 'share' ? (
                    <Share2 aria-hidden size={16} className="text-primary mt-0.5 shrink-0" />
                  ) : (
                    <TriangleAlert aria-hidden size={16} className="text-primary mt-0.5 shrink-0" />
                  )}
                  <span>{t(`settings.torrents.notes.${note}`)}</span>
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setAsking(false)}
                className="hover:bg-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('settings.cancel')}
              </button>
              <button
                type="button"
                onClick={() => {
                  set({ enabled: true });
                  setAsking(false);
                }}
                className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('settings.torrents.confirm')}
              </button>
            </div>
          </div>
        </Sheet>
      )}
    </PrefGroup>
  );
};
