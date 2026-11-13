import { useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useHoldPress } from '../../music/useHoldPress';
import { Popover } from '../../shell/Popover';
import { SpeedSlider } from './SpeedSlider';
import { SPEED_BUBBLES, nextSpeed, speedLabel } from './speed';

/**
 * The reading-aloud bar's speed. A tap steps to the next speed (0.8×, 1×, 1.2×, 1.5×, 2×); a
 * hold, on a phone or with a mouse (or a right click), shows speeds to pick at once and a slider
 * for any speed from 0.5× to 3×. The hold is the button's own: no name shows for it.
 */
export const SpeedButton: FC<{
  /** In hundredths. */
  speed: number;
  onChange: (speed: number) => void;
}> = ({ speed, onChange }) => {
  const { t } = useTranslation();
  const button = useRef<HTMLButtonElement>(null);
  const [picker, setPicker] = useState<HTMLElement | null>(null);
  const press = useHoldPress({
    // With the speeds shown, a tap on the button puts them away.
    onTap: () => (picker ? setPicker(null) : onChange(nextSpeed(speed))),
    onHold: () => setPicker(button.current),
  });
  const label = speedLabel(speed);
  return (
    <>
      <button
        ref={button}
        type="button"
        aria-label={`${t('novel.speed')}: ${label}`}
        aria-haspopup="dialog"
        aria-expanded={picker !== null}
        data-testid="read-aloud-speed"
        {...press.props}
        onContextMenu={(event) => {
          event.preventDefault();
          setPicker(event.currentTarget);
        }}
        className={`min-w-11 rounded-full px-1.5 text-sm font-semibold tabular-nums transition-colors ${
          press.holding || picker ? 'bg-foreground/10' : ''
        }`}
      >
        {label}
      </button>
      {picker && (
        <Popover
          anchor={picker}
          label={t('novel.readAloudOptions.speedPicker')}
          onClose={() => setPicker(null)}
          width={288}
        >
          <div
            role="group"
            aria-label={t('novel.readAloudOptions.speedChoices')}
            className="grid grid-cols-4 gap-2 p-1"
          >
            {SPEED_BUBBLES.map((choice) => (
              <button
                key={choice}
                type="button"
                aria-pressed={choice === speed}
                onClick={() => {
                  onChange(choice);
                  setPicker(null);
                }}
                className={`min-h-10 rounded-full px-2 text-sm font-semibold tabular-nums pointer-coarse:min-h-11 ${
                  choice === speed
                    ? 'bg-primary text-background'
                    : 'bg-background hover:bg-foreground/10'
                }`}
              >
                {speedLabel(choice)}
              </button>
            ))}
          </div>
          <ul className="px-2">
            <SpeedSlider title={t('novel.speed')} value={speed} onChange={onChange} />
          </ul>
        </Popover>
      )}
    </>
  );
};
