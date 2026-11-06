import type { Website } from '@raygun/bindings';
import { Pencil, RotateCcw, ShieldCheck, ShieldOff, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { MenuItem } from '../shell/contextMenuStore';
import { LetterAvatar } from '../shell/LetterAvatar';
import { OverflowMenu } from '../shell/OverflowMenu';
import { SourceIssue } from '../shell/SourceIssue';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { RemoveWebsiteSheet, WebsiteSheet } from './WebsiteSheets';
import { websitePin } from './websitePin';
import { useOpenWebsite, useResetWebsite, useSetWebsiteBlocking } from './useWebsites';

type Item = Extract<MenuItem, { onSelect: () => void }>;
const isItem = (item: MenuItem): item is Item => 'onSelect' in item;

/**
 * Saved websites as rows (M46.3): a tap opens one in the built-in browser; its ⋮ menu (or a
 * right-click) edits, pins to quick access, switches its ad blocker, starts over or removes it.
 */
export const WebsiteList: FC<{ sites: Website[]; large?: boolean }> = ({ sites, large }) => {
  const { t } = useTranslation();
  const open = useOpenWebsite();
  const reset = useResetWebsite();
  const blocking = useSetWebsiteBlocking();
  const pinItems = usePinMenuItem();
  const [editing, setEditing] = useState<Website | null>(null);
  const [removing, setRemoving] = useState<Website | null>(null);
  const opened = sites.find((site) => site.id === open.variables);

  const itemsFor = (site: Website): Item[] => [
    { label: t('websites.edit'), icon: Pencil, onSelect: () => setEditing(site) },
    ...pinItems(websitePin(site)).filter(isItem),
    site.blockAds
      ? {
          label: t('websites.allowHere'),
          icon: ShieldOff,
          onSelect: () => blocking.mutate({ id: site.id, on: false }),
        }
      : {
          label: t('websites.blockHere'),
          icon: ShieldCheck,
          onSelect: () => blocking.mutate({ id: site.id, on: true }),
        },
    ...(site.lastUrl
      ? [{ label: t('websites.startOver'), icon: RotateCcw, onSelect: () => reset.mutate(site.id) }]
      : []),
    { label: t('websites.remove'), icon: Trash2, onSelect: () => setRemoving(site) },
  ];

  return (
    <>
      <ul className="flex flex-col">
        {sites.map((site) => (
          <li key={site.id} data-testid="website-row" className="flex items-center gap-1">
            <button
              type="button"
              aria-label={t('websites.open', { name: site.name })}
              onClick={() => open.mutate(site.id)}
              className={`hover:bg-surface flex min-w-0 flex-1 items-center gap-4 rounded-xl px-1 text-left ${large ? 'py-3' : 'py-2.5'}`}
            >
              <LetterAvatar name={site.name} logo={site.icon} />
              <span className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{site.name}</span>
                <span className="text-muted truncate text-xs">
                  {[site.host, site.blockAds ? null : t('websites.adsAllowed')]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </span>
            </button>
            <OverflowMenu label={t('websites.menu', { name: site.name })} items={itemsFor(site)} />
          </li>
        ))}
      </ul>
      {open.error && (
        <SourceIssue
          compact
          title={t('websites.openFailed', { name: opened?.name ?? t('websites.title') })}
        >
          {open.error.message}
        </SourceIssue>
      )}
      {(reset.error ?? blocking.error) && (
        <SourceIssue compact title={t('websites.saveFailed')}>
          {(reset.error ?? blocking.error)?.message}
        </SourceIssue>
      )}
      {editing && <WebsiteSheet site={editing} onClose={() => setEditing(null)} />}
      {removing && <RemoveWebsiteSheet site={removing} onClose={() => setRemoving(null)} />}
    </>
  );
};
