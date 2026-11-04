import { History } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useClearHistory } from '../../collections/useCollections';
import { ConfirmSheet } from '../ConfirmSheet';
import { PrefAction, PrefGroup, PrefSwitch } from '../prefs';
import { Problem } from '../Problem';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Settings → Privacy: what's remembered of listening (M46.41), and forgetting it. */
export const PrivacySettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const clear = useClearHistory();
  const [confirming, setConfirming] = useState(false);
  if (!data) return null;
  const privacy = data.privacy;
  return (
    <PrefGroup>
      <PrefSwitch
        title={t('settings.privacy.history')}
        checked={privacy.history ?? true}
        onChange={(history) => update.mutate({ key: 'privacy', value: { ...privacy, history } })}
      />
      <PrefSwitch
        title={t('settings.privacy.recentlyPlayed')}
        checked={privacy.recentlyPlayed ?? true}
        onChange={(recentlyPlayed) =>
          update.mutate({ key: 'privacy', value: { ...privacy, recentlyPlayed } })
        }
      />
      <PrefAction
        icon={History}
        danger
        title={t('settings.privacy.clearHistory')}
        summary={clear.isSuccess ? t('settings.privacy.historyCleared') : undefined}
        disabled={clear.isPending}
        onClick={() => setConfirming(true)}
      />
      {clear.isError && (
        <li className="px-1 py-2">
          <Problem title={t('settings.privacy.clearFailed')} detail={clear.error.message} />
        </li>
      )}
      {confirming && (
        <ConfirmSheet
          title={t('settings.privacy.clearHistoryQuestion')}
          confirm={t('settings.privacy.clear')}
          onConfirm={() => clear.mutate(undefined)}
          onClose={() => setConfirming(false)}
        />
      )}
    </PrefGroup>
  );
};
