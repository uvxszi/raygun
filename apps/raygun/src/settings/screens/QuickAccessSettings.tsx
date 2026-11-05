import type { RailKinds, RailOrder } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { RAIL_DEFAULTS, RAIL_ORDERS, RAIL_SHOWN, useRail } from '../../shell/useRail';
import { PrefChoice, PrefGroup, PrefNumber, PrefSwitch } from '../prefs';

/** How many automatic quick access pins are kept (M46.41). */
const AUTO_PIN_LIMITS = [10, 20, 50, 100, 250, 500, 1000];

/** The audio library's parts, as Settings lists them, with the names they have elsewhere. */
const KINDS: { kind: keyof RailKinds; title: string }[] = [
  { kind: 'favorites', title: 'collections.favorites' },
  { kind: 'downloaded', title: 'library.view.downloaded' },
  { kind: 'localFiles', title: 'localFiles.title' },
  { kind: 'playlists', title: 'library.view.playlists' },
  { kind: 'albums', title: 'library.view.albums' },
  { kind: 'artists', title: 'library.view.artists' },
  { kind: 'podcasts', title: 'library.view.podcasts' },
  { kind: 'audiobooks', title: 'library.view.audiobooks' },
];

/**
 * Settings → Quick access: what the desktop sidebar and the phone's drawer list (the audio
 * library, at most so many of it, in an order), whether pins stay on top, and the pins of comics,
 * books, videos and sites added to the library.
 */
export const QuickAccessSettings: FC = () => {
  const { t } = useTranslation();
  const rail = useRail();
  if (!rail.loaded) return null;
  const { prefs } = rail;

  return (
    <>
      <PrefGroup>
        <PrefNumber
          title={t('settings.quickAccess.shown')}
          summary={t('settings.quickAccess.shownHint')}
          value={prefs.shown}
          min={RAIL_SHOWN.min}
          max={RAIL_SHOWN.max}
          defaultValue={RAIL_DEFAULTS.shown}
          format={(count) => t('settings.quickAccess.items', { count })}
          onChange={(shown) => rail.set({ shown })}
        />
        <PrefChoice<RailOrder>
          title={t('settings.quickAccess.order')}
          value={prefs.order}
          options={RAIL_ORDERS.map((value) => ({ value, label: t(`rail.orders.${value}`) }))}
          onChange={(order) => rail.setOrder(order)}
        />
        <PrefSwitch
          title={t('settings.quickAccess.pinnedFirst')}
          summary={t('settings.quickAccess.pinnedFirstHint')}
          checked={prefs.pinnedFirst}
          onChange={(pinnedFirst) => rail.set({ pinnedFirst })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.quickAccess.listed')}>
        {KINDS.map(({ kind, title }) => (
          <PrefSwitch
            key={kind}
            title={t(title)}
            checked={prefs.kinds[kind]}
            onChange={(on) => rail.set({ kinds: { ...prefs.kinds, [kind]: on } })}
          />
        ))}
      </PrefGroup>
      <PrefGroup title={t('settings.quickAccess.otherKinds')}>
        <PrefSwitch
          title={t('settings.quickAccess.autoAdd')}
          summary={t('settings.quickAccess.autoAddHint')}
          checked={prefs.autoAdd}
          onChange={(autoAdd) => rail.set({ autoAdd })}
        />
        <PrefChoice<number>
          title={t('settings.quickAccess.limit')}
          value={prefs.limit}
          disabled={!prefs.autoAdd}
          options={AUTO_PIN_LIMITS.map((value) => ({
            value,
            label: t('settings.quickAccess.pins', { count: value }),
          }))}
          onChange={(limit) => rail.set({ limit })}
        />
      </PrefGroup>
    </>
  );
};
