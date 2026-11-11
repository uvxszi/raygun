import { memo, useLayoutEffect, useRef } from 'react';
import type { FC } from 'react';

import { safeMath } from './mathml';
import './math.css';

type MathFormulaProps = {
  /** `<math>…</math>`, as Rust rebuilt it. */
  mathml: string;
  /** The formula as text, shown when the MathML can't be drawn. */
  text: string;
  /** The formula in words, for screen readers. */
  speech: string;
  /** On a line of its own. */
  display?: boolean;
};

/**
 * A formula drawn by the webview from MathML (MathML Core: Chromium, and so Android's WebView and
 * WebView2, draw it natively) in Latin Modern Math, at the size of the text around it. The markup
 * is checked against an allowlist again before it's placed (before the first paint); a formula
 * that can't be drawn shows its text. React leaves the holder's contents alone.
 */
const MathFormulaView: FC<MathFormulaProps> = ({ mathml, text, speech, display = false }) => {
  const ref = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const holder = ref.current;
    if (!holder) return;
    const math = safeMath(mathml);
    if (math) {
      // Screen readers say the formula in words.
      math.setAttribute('aria-label', speech || text);
      holder.replaceChildren(math);
    } else {
      holder.textContent = text;
    }
  }, [mathml, text, speech]);
  return (
    <span
      ref={ref}
      data-display={display || undefined}
      className={display ? 'math-formula math-display' : 'math-formula'}
    />
  );
};

export const MathFormula = memo(MathFormulaView);
