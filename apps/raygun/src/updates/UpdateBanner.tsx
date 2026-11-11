import { Link } from '@tanstack/react-router';
import { Sparkles, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAppUpdate } from './useAppUpdate';
import { WhatsNewSheet } from './WhatsNewSheet';

const DISMISSED = 'raygun.update.dismissed';

const readDismissed = () => {
  try {
    return localStorage.getItem(DISMISSED);
  } catch {
    return null;
  }
};

/**
 * A newer Raygun is out (M46.47): a line at the top of Home leading to About's update card.
 * What's new opens its notes; × hides it until the next release, as skipping the version does.
 */
export const UpdateBanner: FC = () => {
  const { t } = useTranslation();
  const update = useAppUpdate().data;
  const [dismissed, setDismissed] = useState(readDismissed);
  const [notesOpen, setNotesOpen] = useState(false);
  if (!update?.available || !update.latest || update.skipped || dismissed === update.latest) {
    return null;
  }
  const latest = update.latest;
  return (
    <div
      role="status"
      data-testid="update-banner"
      className="bg-primary/15 flex items-center gap-3 rounded-2xl px-4 py-3 text-sm"
    >
      <Sparkles aria-hidden size={18} className="text-primary shrink-0" />
      <Link to="/about" className="min-w-0 flex-1 font-medium hover:underline">
        {t('updater.banner', { latest })}
      </Link>
      {update.notes && (
        <button
          type="button"
          onClick={() => setNotesOpen(true)}
          className="text-primary shrink-0 font-semibold hover:underline"
        >
          {t('updater.whatsNew')}
        </button>
      )}
      <button
        type="button"
        aria-label={t('updater.dismiss')}
        onClick={() => {
          setDismissed(latest);
          try {
            localStorage.setItem(DISMISSED, latest);
          } catch {
            // A per-device convenience.
          }
        }}
        className="hover:bg-background grid size-8 shrink-0 place-items-center rounded-full"
      >
        <X aria-hidden size={16} />
      </button>
      {notesOpen && update.notes && (
        <WhatsNewSheet
          releases={[{ version: latest, notes: update.notes }]}
          onClose={() => setNotesOpen(false)}
        />
      )}
    </div>
  );
};
