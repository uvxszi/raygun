import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefGroup, PrefSwitch } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';

/** Settings → Browse → Answers: the card in Search → All, and the facts looked up online. */
export const AnswersSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  if (!data) return null;
  const prefs = { inAll: data.answers?.inAll ?? true, online: data.answers?.online ?? true };
  const change = (next: Partial<typeof prefs>) =>
    update.mutate({ key: 'answers', value: { ...prefs, ...next } });
  return (
    <PrefGroup title={t('answers.settings.title')} id="answers">
      <PrefSwitch
        title={t('answers.settings.inAll')}
        summary={t('answers.settings.inAllHint')}
        checked={prefs.inAll}
        onChange={(inAll) => change({ inAll })}
      />
      <PrefSwitch
        title={t('answers.settings.online')}
        summary={t('answers.settings.onlineHint')}
        checked={prefs.online}
        onChange={(online) => change({ online })}
      />
    </PrefGroup>
  );
};
