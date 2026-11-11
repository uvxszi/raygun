import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { languageLabel } from './code';
import './bookText.css';

/**
 * A code example (or other text set out line by line) in the book reader: its lines and spacing as
 * written, in a monospaced font at the reader's text size, scrolling sideways in its own box when
 * a line is wider than the column; the language over it when the source names one.
 */
export const CodeBlock: FC<{
  text: string;
  language?: string | null;
  index: number;
  register: (index: number, element: HTMLElement | null) => void;
}> = ({ text, language, index, register }) => {
  const { t } = useTranslation();
  const name = language ? languageLabel(language) : null;
  return (
    <figure
      ref={(element) => register(index, element)}
      data-block={index}
      aria-label={name ? t('novel.codeIn', { language: name }) : t('novel.code')}
      className="book-code"
    >
      {name && (
        <figcaption aria-hidden className="book-code-language">
          {name}
        </figcaption>
      )}
      <pre>
        <code>{text}</code>
      </pre>
    </figure>
  );
};
