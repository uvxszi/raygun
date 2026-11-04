import { X } from 'lucide-react';
import { useContext, useId, useRef } from 'react';
import type { FC, ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { useBackLayer } from './backLayers';
import { Popover } from './Popover';
import { SheetAboveContext } from './sheetAbove';
import { useWideScreen } from './useWideScreen';

type SheetProps = {
  title: string;
  onClose: () => void;
  children: ReactNode;
  /**
   * The button that opened it: on wide windows the sheet is then a popover beside that button
   * (M46.7) instead of a dialog over the whole window.
   */
  anchor?: HTMLElement | null;
  /** The popover's width in px. */
  popoverWidth?: number;
};

/**
 * A bottom sheet (a centred dialog on wide screens, or a popover beside the button that opened
 * it) for short tasks: typing an address, naming a playlist, picking a folder. Inputs live here or
 * in the top bar, never at the end of a list.
 */
export const Sheet: FC<SheetProps> = ({ anchor, popoverWidth, ...props }) => {
  const wide = useWideScreen();
  if (anchor && wide) {
    return (
      <Popover anchor={anchor} label={props.title} onClose={props.onClose} width={popoverWidth}>
        <h2 className="text-muted px-3 pt-1 pb-2 text-xs font-semibold tracking-wide uppercase">
          {props.title}
        </h2>
        {props.children}
      </Popover>
    );
  }
  return <DialogSheet {...props} />;
};

const DialogSheet: FC<Omit<SheetProps, 'anchor' | 'popoverWidth'>> = ({
  title,
  onClose,
  children,
}) => {
  const { t } = useTranslation();
  const titleId = useId();
  // Closing on pointer down would let the tap's click land on whatever is under the sheet.
  const pressedBackdrop = useRef(false);
  // Back and Escape close it, and only it (not a sheet under it).
  useBackLayer(true, onClose);
  const above = useContext(SheetAboveContext);

  // On the document's body, so a transformed or scrolling ancestor never carries it along.
  return createPortal(
    <div
      className={`motion-fade fixed inset-0 flex items-end justify-center bg-black/60 sm:items-center ${above ? 'z-[80]' : 'z-50'}`}
      onPointerDown={(event) => {
        pressedBackdrop.current = event.target === event.currentTarget;
      }}
      onClick={(event) => {
        if (pressedBackdrop.current && event.target === event.currentTarget) onClose();
        pressedBackdrop.current = false;
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="bg-surface motion-rise flex max-h-[85dvh] w-full flex-col overflow-hidden rounded-t-3xl pt-5 sm:max-w-md sm:rounded-3xl"
      >
        {/* The title stays put while a long sheet scrolls under it. */}
        <div className="flex shrink-0 items-center justify-between gap-3 px-5 pb-2">
          <h2 id={titleId} className="text-lg font-semibold">
            {title}
          </h2>
          <button
            type="button"
            aria-label={t('watch.close')}
            onClick={onClose}
            className="hover:bg-background -mr-2 grid size-10 place-items-center rounded-full"
          >
            <X aria-hidden size={20} />
          </button>
        </div>
        <div
          data-testid="sheet-body"
          className="flex min-h-0 flex-1 touch-pan-y flex-col gap-4 overflow-y-auto overscroll-contain px-5 pt-2 pb-[calc(1.25rem+env(safe-area-inset-bottom))] [-webkit-overflow-scrolling:touch]"
        >
          {children}
        </div>
      </div>
    </div>,
    document.body,
  );
};
