import { LayoutList, Link2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useLayout } from '../layout/useLayout';
import { OpenLinkSheet } from '../pages/OpenLinkSheet';
import { useIncognitoItem } from '../privacy/useIncognito';
import { OverflowMenu } from '../shell/OverflowMenu';
import type { OverflowItem } from '../shell/OverflowMenu';
import { Tabs } from '../shell/Tabs';
import { TopBar } from '../shell/TopBar';
import type { TopBarSearch } from '../shell/TopBar';

/**
 * All (M46.37), Audio (music and podcasts), Comics (manga, manhwa, comics), Books (novels,
 * books), Videos.
 */
export type LibraryMediaTab = 'all' | 'music' | 'manga' | 'novel' | 'video';

const isMediaTab = (value: string): value is LibraryMediaTab =>
  value === 'all' ||
  value === 'music' ||
  value === 'manga' ||
  value === 'novel' ||
  value === 'video';

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
  const layout = useLayout();
  const incognitoItem = useIncognitoItem();
  // The user's tabs in their order (M46.5); one opened from elsewhere shows while it's open.
  const tabs = layout.libraryMedia
    .filter((item) => item.visible || item.id === media)
    .map((item) => item.id)
    .filter(isMediaTab);
  const first = layout.libraryMedia.find((item) => item.visible)?.id;

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
                incognitoItem,
                {
                  label: t('layout.editLibraryTabs'),
                  icon: LayoutList,
                  to: '/settings/layout/libraryMedia',
                },
              ]}
            />
          </>
        }
      />
      <Tabs
        label={t('library.media.label')}
        active={media}
        items={tabs.map((value) => ({
          id: value,
          label: t(`library.tabs.${value}`),
          to: '/library',
          // The first tab is the Library's own address; the others say which they are.
          search: value === first ? {} : { media: value },
          testId: `library-media-${value}`,
        }))}
      />
      {opening && (
        <OpenLinkSheet
          initialKind={media === 'music' || media === 'all' ? 'audio' : 'video'}
          onClose={() => setOpening(false)}
        />
      )}
    </>
  );
};
