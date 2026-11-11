import { memo } from 'react';
import type { CSSProperties, FC } from 'react';

import { bookCoverLook, titleLength } from './bookWords';
import type { BookWords } from './bookWords';

const BookCoverView: FC<BookWords> = ({ title, author }) => {
  const look = bookCoverLook(title);
  const style = { background: look.background, '--book-accent': look.accent } as CSSProperties;
  const words = title.trim();
  // The words are drawn by CSS (`content: attr(data-text)`): the page's text keeps the title
  // once, where the card prints it, and nothing reads it twice.
  return (
    <span aria-hidden data-testid="book-cover" className="book-cover" style={style}>
      <span className="book-cover-title" data-text={words} data-length={titleLength(words)} />
      <span className="book-cover-rule" />
      {author && <span className="book-cover-author" data-text={author} />}
      <span className="book-cover-initial" data-text={Array.from(words)[0]?.toUpperCase() ?? ''} />
    </span>
  );
};

/**
 * A cover drawn for a book that has none, or under its own until that loads: the title near the
 * top and the author at the foot, set like a printed cover on colours of the title's own
 * (`bookWords`). Plain HTML and CSS (`.book-cover` in styles.css) sized by the cover's width, a
 * few empty spans in a long grid; too narrow for words, it shows the title's first letter.
 */
export const BookCover = memo(BookCoverView);
