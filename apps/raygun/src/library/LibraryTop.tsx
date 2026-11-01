import { Link2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { OpenLinkSheet } from '../pages/OpenLinkSheet';
import { OverflowMenu } from '../shell/OverflowMenu';
import type { OverflowItem } from '../shell/OverflowMenu';
import { Tabs } from '../shell/Tabs';
import { TopBar } from '../shell/TopBar';
import type { TopBarSearch } from '../shell/TopBar';

export type LibraryMediaTab = 'music' | 'podcast' | 'manga' | 'novel' | 'video';

const MEDIA: LibraryMediaTab[] = ['music', 'podcast', 'manga', 'novel', 'video'];

/**
 * The Library's top bar and media tabs. Each media view passes its own actions and menu items;
 * every one gets "Open link or file", so no view needs a form of its own.
 */
export const LibraryTop: FC<{
  media: LibraryMediaTab;
  actions?: ReactNode;
  menu?: OverflowItem[];
  search?: TopBarSearch | null;
}> = ({ media, actions, menu = [], search }) => {
  const { t } = useTranslation();
  const [opening, setOpening] = useState(false);

  return (
    <>
      <TopBar
        title={t('library.title')}
        search={search}
        actions={
          <>
            {actions}
            <OverflowMenu
              items={[
                ...menu,
                { label: t('library.openTitle'), icon: Link2, onSelect: () => setOpening(true) },
              ]}
            />
          </>
        }
      />
      <Tabs
        label={t('library.media.label')}
        active={media}
        items={MEDIA.map((value) => ({
          id: value,
          label: t(`library.media.${value}`),
          to: '/library',
          search: value === 'music' ? {} : { media: value },
          testId: `library-media-${value}`,
        }))}
      />
      {opening && (
        <OpenLinkSheet
          initialKind={media === 'music' || media === 'podcast' ? 'audio' : 'video'}
          onClose={() => setOpening(false)}
        />
      )}
    </>
  );
};
