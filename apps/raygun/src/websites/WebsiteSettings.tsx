import type { Blocklist, WebsitePrefs } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlatform } from '../lib/usePlatform';
import { PrefBlock, PrefChoice, PrefGroup, PrefSwitch } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { useBlocklist, useClearWebsiteData, useUpdateBlocklist } from './useWebsites';

const LISTS: Blocklist[] = ['hageziMulti', 'hageziLight', 'hageziPro', 'stevenBlack', 'custom'];

type Prefs = Required<WebsitePrefs>;

/** Matches `WebsitePrefs::default()` in raygun-model. */
const WEBSITE_DEFAULTS: Prefs = {
  blockAds: true,
  blockPopups: true,
  blocklist: 'hageziMulti',
  customListUrl: null,
  desktop: false,
};

/** Settings → Browse → Websites (M46.3): the built-in browser's blocker and data. */
export const WebsiteSettings: FC = () => {
  const { t, i18n } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const status = useBlocklist();
  const refresh = useUpdateBlocklist();
  const clear = useClearWebsiteData();
  const [customUrl, setCustomUrl] = useState<string | null>(null);
  // Desktop sites are desktop sites already.
  const android = (usePlatform() ?? 'android') === 'android';
  const prefs: Prefs = { ...WEBSITE_DEFAULTS, ...settings.data?.websites };
  if (!settings.data) return null;
  const busy = update.isPending;
  const change = (patch: Partial<Prefs>) =>
    update.mutate({ key: 'websites', value: { ...prefs, ...patch } });
  const listed = status.data?.domains ?? 0;
  const updatedAt = status.data?.updatedAt;
  const listSummary =
    listed > 0 && updatedAt
      ? t('websites.listStatus', {
          count: listed,
          formatted: listed.toLocaleString(i18n.language),
          date: new Date(updatedAt).toLocaleDateString(i18n.language),
        })
      : t('websites.listNone');

  return (
    <PrefGroup title={t('websites.title')}>
      <PrefSwitch
        title={t('websites.blockAds')}
        summary={t('websites.blockAdsHint')}
        checked={prefs.blockAds}
        disabled={busy}
        onChange={(blockAds) => change({ blockAds })}
      />
      <PrefSwitch
        title={t('websites.blockPopups')}
        summary={t('websites.blockPopupsHint')}
        checked={prefs.blockPopups}
        disabled={busy}
        onChange={(blockPopups) => change({ blockPopups })}
      />
      <PrefChoice<Blocklist>
        title={t('websites.list')}
        summary={listSummary}
        value={prefs.blocklist}
        disabled={busy || !prefs.blockAds}
        options={LISTS.map((value) => ({
          value,
          label: t(`websites.lists.${value}`),
          hint: t(`websites.listHints.${value}`),
        }))}
        onChange={(blocklist) => {
          if (blocklist === 'custom') setCustomUrl(prefs.customListUrl ?? '');
          else change({ blocklist });
        }}
      />
      <PrefBlock>
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={refresh.isPending || !prefs.blockAds}
            onClick={() => refresh.mutate()}
            className="bg-surface rounded-full px-4 py-2 text-sm font-medium disabled:opacity-50"
          >
            {refresh.isPending ? t('websites.updating') : t('websites.updateList')}
          </button>
          <button
            type="button"
            disabled={clear.isPending}
            onClick={() => clear.mutate()}
            className="bg-surface rounded-full px-4 py-2 text-sm font-medium disabled:opacity-50"
          >
            {t('websites.clearData')}
          </button>
        </div>
        {(refresh.isError || status.data?.error) && (
          <p role="alert" className="text-primary mt-2 text-sm wrap-anywhere">
            {refresh.error?.message ?? status.data?.error}
          </p>
        )}
        {clear.isSuccess && (
          <p role="status" className="text-muted mt-2 text-sm">
            {t('websites.cleared')}
          </p>
        )}
      </PrefBlock>
      {android && (
        <PrefSwitch
          title={t('websites.desktop')}
          summary={t('websites.desktopHint')}
          checked={prefs.desktop}
          disabled={busy}
          onChange={(desktop) => change({ desktop })}
        />
      )}
      {customUrl !== null && (
        <Sheet title={t('websites.lists.custom')} onClose={() => setCustomUrl(null)}>
          <form
            className="flex flex-col gap-3 text-sm"
            onSubmit={(event) => {
              event.preventDefault();
              update.mutate(
                {
                  key: 'websites',
                  value: { ...prefs, blocklist: 'custom', customListUrl: customUrl.trim() },
                },
                { onSuccess: () => setCustomUrl(null) },
              );
            }}
          >
            <TextField
              type="text"
              inputMode="url"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              autoFocus
              value={customUrl}
              onChange={setCustomUrl}
              placeholder="https://…/list.txt"
              aria-label={t('websites.customUrl')}
              className={SHEET_FIELD}
              inputClassName={SHEET_INPUT}
            />
            <p className="text-muted text-xs">{t('websites.customHint')}</p>
            {update.isError && (
              <p role="alert" className="text-primary text-xs">
                {update.error.message}
              </p>
            )}
            <button
              type="submit"
              disabled={!customUrl.trim() || busy}
              className="bg-primary text-background self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
            >
              {t('websites.save')}
            </button>
          </form>
        </Sheet>
      )}
    </PrefGroup>
  );
};
