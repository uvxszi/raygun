import { Link } from '@tanstack/react-router';
import {
  Ban,
  Check,
  CircleCheck,
  Compass,
  Eraser,
  Filter,
  Globe,
  MoreHorizontal,
  Settings2,
} from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useContentRating } from '../extensions/contentRating';
import {
  useClearExtensionData,
  useExtension,
  useOpenExtensionWebsite,
} from '../extensions/useExtensions';
import { Sheet } from '../shell/Sheet';
import { useSourceSwitch } from './useSourceSwitch';

const ITEM =
  'hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm';

type SourceMenuProps = {
  sourceId: string;
  extensionId: string;
  name: string;
  /** The source's own page, when it has one to browse. */
  page?: { to: string; search: Record<string, unknown> };
  /** Narrows the search to this source. */
  onOnly?: () => void;
};

/**
 * A source's ⋯ beside its name in Browse and Search (M46.8): search only it, open it, its
 * extension's settings, or switch it off, without hunting for it in Sources. M46.11 adds its
 * website in Raygun's browser and clearing its cookies and stored data.
 */
export const SourceMenu: FC<SourceMenuProps> = ({ sourceId, extensionId, name, page, onOnly }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const { enabled, setEnabled, busy } = useSourceSwitch(sourceId);
  const extension = useExtension(extensionId).data;
  const website = extension?.website ?? null;
  const rating = useContentRating();
  const openWebsite = useOpenExtensionWebsite();
  const clearData = useClearExtensionData();
  const close = () => {
    setAnchor(null);
    clearData.reset();
  };
  return (
    <>
      <button
        type="button"
        aria-label={t('sources.menu', { name })}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full"
      >
        <MoreHorizontal aria-hidden size={18} />
      </button>
      {anchor && (
        <Sheet title={name} anchor={anchor} popoverWidth={260} onClose={close}>
          <div className="-mx-2 flex flex-col">
            {onOnly && (
              <button
                type="button"
                onClick={() => {
                  onOnly();
                  close();
                }}
                className={ITEM}
              >
                <Filter aria-hidden size={16} className="text-muted" />
                {t('sources.only')}
              </button>
            )}
            {page && (
              <Link to={page.to} search={page.search} onClick={close} className={ITEM}>
                <Compass aria-hidden size={16} className="text-muted" />
                {t('sources.open')}
              </Link>
            )}
            <Link
              to="/extensions/detail"
              search={{ id: extensionId }}
              onClick={close}
              className={ITEM}
            >
              <Settings2 aria-hidden size={16} className="text-muted" />
              {t('sources.settings')}
            </Link>
            {website && (
              <button
                type="button"
                onClick={() => {
                  openWebsite.mutate(extensionId);
                  close();
                }}
                className={ITEM}
              >
                <Globe aria-hidden size={16} className="text-muted" />
                {t('sources.website')}
              </button>
            )}
            <button
              type="button"
              disabled={clearData.isPending || clearData.isSuccess}
              onClick={() => clearData.mutate(extensionId)}
              className={`${ITEM} disabled:opacity-70`}
            >
              {clearData.isSuccess ? (
                <Check aria-hidden size={16} className="text-primary" />
              ) : (
                <Eraser aria-hidden size={16} className="text-muted" />
              )}
              {t(clearData.isSuccess ? 'sources.cleared' : 'sources.clearData')}
            </button>
            {/* Marks the whole extension, so its other sources follow. */}
            {rating
              .menuItems('extensions', extensionId, extension?.adult ?? false)
              .map(({ label, icon: Icon, onSelect }) => (
                <button
                  key={label}
                  type="button"
                  onClick={() => {
                    onSelect?.();
                    close();
                  }}
                  className={ITEM}
                >
                  {Icon && <Icon aria-hidden size={16} className="text-muted" />}
                  {label}
                </button>
              ))}
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                setEnabled(!enabled);
                close();
              }}
              className={`${ITEM} ${enabled ? 'text-primary' : ''}`}
            >
              {enabled ? (
                <Ban aria-hidden size={16} />
              ) : (
                <CircleCheck aria-hidden size={16} className="text-muted" />
              )}
              {t(enabled ? 'sources.disable' : 'sources.enable')}
            </button>
          </div>
        </Sheet>
      )}
    </>
  );
};
