import type { Website } from '@raygun/bindings';
import { Globe } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { usePlaylistMenuItems } from '../collections/usePlaylistMenu';
import { useTitleCardMenu } from '../manga/titleMenu';
import { useBookMenuItems, usePodcastMenuItems } from '../music/cardMenus';
import { withSelect } from '../select/withSelect';
import type { MenuItem } from '../shell/contextMenuStore';
import { useWebsiteMenuItems } from '../websites/useWebsiteMenu';
import type { AllItem } from './allItems';

/** What a site's menu opens: the browser, and the websites' own sheets. */
export type SiteActions = {
  open: (site: Website) => void;
  edit: (site: Website) => void;
  remove: (site: Website) => void;
};

/**
 * The menu of an item of Library → All, the same as its kind's elsewhere (a playlist's, a show's,
 * a book's, a title's, a site's), with Select first while it isn't picked (`select`).
 */
export const useAllItemMenu = (sites: SiteActions) => {
  const { t } = useTranslation();
  const playlistMenu = usePlaylistMenuItems();
  const podcastMenu = usePodcastMenuItems();
  const bookMenu = useBookMenuItems();
  const titleMenu = useTitleCardMenu();
  const websiteMenu = useWebsiteMenuItems();

  const own = (item: AllItem): MenuItem[] => {
    switch (item.kind) {
      case 'playlist':
        return playlistMenu(item.playlist);
      case 'podcast':
        return podcastMenu({
          title: item.show.title,
          author: item.show.author,
          feedUrl: item.show.feedUrl,
          artworkUrl: item.show.artworkUrl,
          id: item.show.id,
          subscribed: true,
        });
      case 'audiobook':
        return bookMenu({
          title: item.book.title,
          author: item.book.author,
          artworkUrl: item.book.artworkUrl,
          id: item.book.id,
          kept: true,
        });
      case 'title':
        return titleMenu({
          medium: item.medium,
          sourceId: item.entry.sourceId,
          id: item.entry.remoteId,
          title: item.entry.title,
          coverUrl: item.entry.coverUrl,
          subtitle: item.entry.authors.join(', ') || null,
          inLibrary: true,
          titleId: item.entry.titleId,
        });
      case 'website': {
        const site = item.site;
        return [
          { kind: 'heading', label: site.name, subtitle: site.host },
          { label: t('menu.open'), icon: Globe, onSelect: () => sites.open(site) },
          { kind: 'separator' },
          ...websiteMenu.items(site, {
            edit: () => sites.edit(site),
            remove: () => sites.remove(site),
          }),
        ];
      }
    }
  };

  return (item: AllItem, select: (() => void) | null): MenuItem[] =>
    withSelect(own(item), t, select);
};
