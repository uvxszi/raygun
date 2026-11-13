import type { FC } from 'react';

import { PrefSlider } from '../../settings/prefs';
import { NORMAL_SPEED, SPEED_MAX, SPEED_MIN, SPEED_STEP, speedLabel } from './speed';

/** The reading-aloud speed as a settings row: 0.5× to 3× in steps of 0.05. */
export const SpeedSlider: FC<{
  title: string;
  /** In hundredths; none stored is the normal speed. */
  value: number | undefined;
  onChange: (speed: number) => void;
  disabled?: boolean;
}> = ({ title, value, onChange, disabled }) => (
  <PrefSlider
    title={title}
    value={value ?? NORMAL_SPEED}
    min={SPEED_MIN}
    max={SPEED_MAX}
    step={SPEED_STEP}
    format={speedLabel}
    disabled={disabled}
    onChange={onChange}
  />
);
