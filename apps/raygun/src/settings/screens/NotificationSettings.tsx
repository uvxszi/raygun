import type { NotificationPrefs } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefGroup, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Which news posts a notification; downloads and progress are always silent. */
const KINDS = [
  'artistReleases',
  'comics',
  'books',
  'videos',
  'podcasts',
  'audiobooks',
  'appUpdates',
] as const satisfies readonly (keyof NotificationPrefs)[];

/** Settings → Notifications: one switch per kind of news. */
export const NotificationSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  if (!data) return null;
  const prefs = data.notifications;
  return (
    <PrefGroup>
      <li className="text-muted px-1 pb-2 text-xs">{t('settings.notifications.hint')}</li>
      {KINDS.map((kind) => (
        <PrefSwitch
          key={kind}
          title={t(`settings.notifications.${kind}`)}
          checked={prefs?.[kind] ?? true}
          onChange={(on) =>
            update.mutate({ key: 'notifications', value: { ...prefs, [kind]: on } })
          }
        />
      ))}
    </PrefGroup>
  );
};
