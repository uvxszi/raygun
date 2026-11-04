import type { ReleaseNotes } from '@raygun/bindings';
import { commands } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { Markdown } from './Markdown';
import { useOpenNotesLink, usePendingNotes } from './useReleaseNotes';

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

/**
 * The first start after an update shows that version's notes once; closing the sheet remembers
 * it (`local_meta`).
 */
export const WhatsNewOnStart: FC = () => {
  const [closed, setClosed] = useState(false);
  const release = usePendingNotes().data;
  if (closed || !release) return null;
  return (
    <WhatsNewSheet
      releases={[release]}
      onClose={() => {
        setClosed(true);
        void commands.appWhatsNewSeen();
      }}
    />
  );
};
