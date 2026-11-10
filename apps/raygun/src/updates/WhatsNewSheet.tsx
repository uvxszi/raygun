import type { ReleaseNotes } from '@raygun/bindings';
import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { Markdown } from './Markdown';
import { PENDING_NOTES_KEY, useOpenNotesLink, usePendingNotes } from './useReleaseNotes';

type WhatsNewSheetProps = {
  /** Newest first; with more than one, chips switch between them. */
  releases: ReleaseNotes[];
  onClose: () => void;
};

/** "What's new in <version>": a release's notes, rendered. */
export const WhatsNewSheet: FC<WhatsNewSheetProps> = ({ releases, onClose }) => {
  const { t } = useTranslation();
  const openLink = useOpenNotesLink();
  const [shown, setShown] = useState(0);
  const release = releases[shown] ?? releases[0];
  if (!release) return null;
  return (
    <Sheet title={t('updater.whatsNewIn', { version: release.version })} onClose={onClose}>
      {releases.length > 1 && (
        <div role="group" aria-label={t('updater.versions')} className="flex flex-wrap gap-2">
          {releases.map((entry, position) => (
            <button
              key={entry.version}
              type="button"
              aria-pressed={position === shown}
              onClick={() => setShown(position)}
              className={`rounded-full px-3 py-1 text-sm ${position === shown ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
            >
              {entry.version}
            </button>
          ))}
        </div>
      )}
      <Markdown source={release.notes} onOpenLink={openLink} />
    </Sheet>
  );
};

/** When this run started: answers older than this came from the saved cache, not from Rust now. */
const RUN_STARTED_AT = Date.now();

/**
 * The first start after an update shows that version's notes once; closing the sheet remembers
 * it (`local_meta`). Only Rust's answer in this run counts: one restored from an earlier run
 * (shell/queryPersist) may be about notes since seen, and would show them for an instant.
 */
export const WhatsNewOnStart: FC = () => {
  const queryClient = useQueryClient();
  const [closed, setClosed] = useState(false);
  const pending = usePendingNotes();
  const release = pending.data;
  if (closed || !release || pending.dataUpdatedAt < RUN_STARTED_AT) return null;
  return (
    <WhatsNewSheet
      releases={[release]}
      onClose={() => {
        setClosed(true);
        queryClient.setQueryData(PENDING_NOTES_KEY, null);
        void commands.appWhatsNewSeen();
      }}
    />
  );
};
