import { useState } from 'react';
import type { FC, SVGProps } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from './Sheet';

/**
 * An anime girl's face in the line style of the other icons (M46.24): 24 × 24, a 2 px round
 * stroke, with a fringe, twin tails, two eyes and a smile.
 */
export const MascotIcon: FC<SVGProps<SVGSVGElement> & { size?: number }> = ({
  size = 24,
  ...props
}) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={2}
    strokeLinecap="round"
    strokeLinejoin="round"
    {...props}
  >
    {/* The head's top and the face below it. */}
    <path d="M6 13a6 6 0 0 1 12 0" />
    <path d="M7 13a5 5 0 0 0 10 0" />
    {/* The fringe. */}
    <path d="M7.5 11.5c2 0 3.5-1.5 4.5-3.5 1 2 2.5 3.5 4.5 3.5" />
    {/* Twin tails. */}
    <path d="M6 10.5c-2 .5-3 3-2.5 6.5" />
    <path d="M18 10.5c2 .5 3 3 2.5 6.5" />
    {/* Eyes and a smile. */}
    <path d="M10 14v1" />
    <path d="M14 14v1" />
    <path d="M11 17.25c.6.4 1.4.4 2 0" />
  </svg>
);

/**
 * The mascot's button (M46.24), left of Updates on desktop and of Search on the phone's Home:
 * mascots that dance and sing to what's playing are coming, and for now it says so.
 */
export const MascotButton: FC<{ className: string; iconSize: number }> = ({
  className,
  iconSize,
}) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button
        type="button"
        aria-label={t('mascot.label')}
        data-hint={t('mascot.label')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={className}
      >
        <MascotIcon aria-hidden size={iconSize} />
      </button>
      {anchor && (
        <Sheet
          title={t('mascot.label')}
          anchor={anchor}
          popoverWidth={280}
          onClose={() => setAnchor(null)}
        >
          <div data-testid="mascot-soon" className="flex items-center gap-3 px-3 pb-3">
            <span className="bg-primary/15 text-primary grid size-12 shrink-0 place-items-center rounded-full">
              <MascotIcon aria-hidden size={26} />
            </span>
            <span className="flex flex-col gap-0.5">
              <span className="font-semibold">{t('mascot.soon')}</span>
              <span className="text-muted text-sm">{t('mascot.what')}</span>
            </span>
          </div>
        </Sheet>
      )}
    </>
  );
};
