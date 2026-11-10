import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Popover } from '../shell/Popover';

const BUTTON = 'rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60';

/** What the question says: a download loses what it saved, an export keeps the files it wrote. */
export type StopKind = 'download' | 'export';

/**
 * Stopping asks first, in place: a small popover beside the control that was pressed, with Keep
 * downloading and Stop. Every download's progress ring, spinner or Cancel uses it.
 */
export const StopDownloadPopover: FC<{
  anchor: HTMLElement;
  name: string;
  kind?: StopKind;
  onStop: () => void;
  onClose: () => void;
}> = ({ anchor, name, kind = 'download', onStop, onClose }) => {
  const { t } = useTranslation();
  const title = t(kind === 'export' ? 'downloads.stop.exportTitle' : 'downloads.stop.title');
  return (
    <Popover anchor={anchor} label={title} onClose={onClose} width={300}>
      <div data-testid="stop-download" className="flex flex-col gap-3 p-4">
        <p className="font-semibold">{title}</p>
        <p className="text-muted text-sm wrap-anywhere">
          {t(kind === 'export' ? 'downloads.stop.exportHint' : 'downloads.stop.hint', { name })}
        </p>
        <div className="flex flex-wrap justify-end gap-2">
          <button type="button" onClick={onClose} className={`${BUTTON} bg-background`}>
            {t(kind === 'export' ? 'downloads.stop.keepExport' : 'downloads.stop.keep')}
          </button>
          <button
            type="button"
            onClick={() => {
              onClose();
              onStop();
            }}
            className={`${BUTTON} bg-primary text-background`}
          >
            {t('downloads.stop.stop')}
          </button>
        </div>
      </div>
    </Popover>
  );
};

/** A download's ring, spinner or Cancel: pressing it asks before stopping. */
export const StopDownloadButton: FC<{
  /** The button's own name ("Downloading Episode 3, 40%. Stop"). */
  label: string;
  /** What's downloading, for the question. */
  name: string;
  kind?: StopKind;
  disabled?: boolean;
  className: string;
  onStop: () => void;
  children: ReactNode;
}> = ({ label, name, kind, disabled = false, className, onStop, children }) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <button
        type="button"
        aria-label={label}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        disabled={disabled}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={className}
      >
        {children}
      </button>
      {anchor && (
        <StopDownloadPopover
          anchor={anchor}
          name={name}
          kind={kind}
          onStop={onStop}
          onClose={() => setAnchor(null)}
        />
      )}
    </>
  );
};
