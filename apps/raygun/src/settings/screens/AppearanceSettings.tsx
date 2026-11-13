import { LayoutList } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { VideoCardsGroup } from '../../watch/VideoCardChoices';
import { AppearanceCard } from '../AppearanceCard';
import { PrefGroup, PrefLink } from '../prefs';

/** Settings → Appearance: the card, and the way to arrange tabs and screens (M46.5). */
export const AppearanceSettings: FC = () => {
  const { t } = useTranslation();
  return (
    <>
      <AppearanceCard />
      <VideoCardsGroup />
      <PrefGroup>
        <PrefLink title={t('layout.title')} to="/settings/layout" icon={LayoutList} />
      </PrefGroup>
    </>
  );
};
