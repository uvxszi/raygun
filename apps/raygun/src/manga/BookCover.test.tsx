import { fireEvent, screen, within } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';
import { renderWithProviders } from '../test/renderWithProviders';
import { bookCoverLook, bookWords } from './bookWords';
import { MangaCover } from './MangaCover';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** The words a drawn cover sets: CSS prints each part's `data-text`. */
const words = (cover: HTMLElement) =>
  Object.fromEntries(
    ['title', 'author', 'initial'].map((part) => [
      part,
      cover.querySelector(`.book-cover-${part}`)?.getAttribute('data-text') ?? null,
    ]),
  );

describe('Drawn book covers', () => {
  it('draws a book without a cover from its title and author', () => {
    renderWithProviders(
      <MangaCover
        url={null}
        book={{ title: 'Pride and Prejudice', author: 'Jane Austen' }}
        className="w-full"
      />,
    );

    const cover = screen.getByTestId('book-cover');
    expect(cover).toHaveAttribute('aria-hidden', 'true');
    expect(words(cover)).toEqual({
      title: 'Pride and Prejudice',
      author: 'Jane Austen',
      initial: 'P',
    });
    // The words are drawn, not text of the page: the card's own title stays the only one.
    expect(screen.queryByText('Pride and Prejudice')).not.toBeInTheDocument();
    expect(screen.getByTestId('book-frame').querySelector('img')).toBeNull();
  });

  it("keeps the drawn cover under the book's own until it loads, and when it fails", () => {
    renderWithProviders(
      <MangaCover url="http://127.0.0.1:1/cover/emma" book={{ title: 'Emma' }} />,
    );

    const frame = screen.getByTestId('book-frame');
    const image = frame.querySelector('img');
    expect(image).toHaveAttribute('src', 'http://127.0.0.1:1/cover/emma');
    expect(within(frame).getByTestId('book-cover')).toBeInTheDocument();
    expect(image).not.toHaveAttribute('data-loaded');

    fireEvent.load(image!);
    expect(image).toHaveAttribute('data-loaded');

    fireEvent.error(image!);
    expect(frame.querySelector('img')).toBeNull();
    expect(words(within(frame).getByTestId('book-cover')).title).toBe('Emma');
  });

  it('gives each title colours of its own, the same every time', () => {
    expect(bookCoverLook('Emma')).toEqual(bookCoverLook('Emma'));
    expect(bookCoverLook('Emma').background).not.toBe(bookCoverLook('Persuasion').background);
    expect(bookCoverLook('Emma').background).toMatch(/^linear-gradient\(/);
  });

  it('draws covers for books only', () => {
    expect(bookWords('novel', 'Emma', ['Jane Austen', 'An Editor'])).toEqual({
      title: 'Emma',
      author: 'Jane Austen, An Editor',
    });
    expect(bookWords('novel', 'Emma', [])).toEqual({ title: 'Emma', author: null });
    expect(bookWords('manga', 'Berserk', ['Kentaro Miura'])).toBeUndefined();

    renderWithProviders(<MangaCover url={null} className="w-full" />);
    expect(screen.queryByTestId('book-cover')).not.toBeInTheDocument();
  });

  it("draws the library's books that have no cover, and the others' until theirs load", async () => {
    AppShellWrapper.seedCore();
    commands.novelLibrary.mockResolvedValue(
      ok([
        libraryEntry({
          titleId: 'f',
          remoteId: 'f',
          title: 'Frankenstein',
          authors: ['Mary Shelley'],
        }),
        libraryEntry({
          titleId: 'd',
          remoteId: 'd',
          title: 'Dracula',
          coverUrl: 'http://127.0.0.1:1/cover/dracula',
        }),
      ]),
    );
    await AppShellWrapper.mount('/library?media=novel');

    const library = await screen.findByTestId('novel-library');
    const items = await within(library).findAllByTestId('library-item');
    const item = (title: string) =>
      items.find((each) => each.getAttribute('aria-label') === title)!;

    const frankenstein = item('Frankenstein');
    expect(words(within(frankenstein).getByTestId('book-cover'))).toMatchObject({
      title: 'Frankenstein',
      author: 'Mary Shelley',
    });
    // Its title is in the card once, however the library lays covers out.
    expect(within(frankenstein).getAllByText('Frankenstein')).toHaveLength(1);

    const dracula = item('Dracula');
    expect(dracula.querySelector('img')).toHaveAttribute('src', 'http://127.0.0.1:1/cover/dracula');
    expect(within(dracula).getByTestId('book-cover')).toBeInTheDocument();
  });
});
