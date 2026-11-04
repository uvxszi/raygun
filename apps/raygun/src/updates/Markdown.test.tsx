import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import RELEASE from '../../../../docs/releases/v1.1.0.md?raw';
import { Markdown } from './Markdown';
import { parseInline, parseMarkdown } from './markdownParse';

describe('Markdown', () => {
  it('renders the 1.1.0 release notes', () => {
    render(<Markdown source={RELEASE} onOpenLink={vi.fn()} />);
    expect(screen.getByRole('heading', { name: "What's in this release" })).toBeInTheDocument();
    const playback = screen.getByText('Playback');
    expect(playback.tagName).toBe('STRONG');
    const lists = screen.getAllByRole('list');
    expect(lists).toHaveLength(7);
    expect(within(lists[0] as HTMLElement).getAllByRole('listitem')).toHaveLength(5);
    expect(
      screen.getByText(/Music keeps playing in the background on Android/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/\*\*/)).not.toBeInTheDocument();
    expect(screen.queryByText(/^- /)).not.toBeInTheDocument();
  });

  it('renders inline code, emphasis, links and fenced code, and opens https links outside', async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    render(
      <Markdown
        source={[
          '# Title',
          'Run `pnpm dev`, *really* ~~not~~ __now__. See [the PR](https://github.com/uvxszi/raygun/pull/1).',
          '',
          '```sh',
          'cargo test',
          '```',
          '',
          '1. one',
          '2. two',
          '   - nested',
          '',
          '> quoted',
          '',
          '| A | B |',
          '| - | - |',
          '| 1 | 2 |',
          '',
          '[bad](javascript:alert(1)) <script>alert(1)</script>',
        ].join('\n')}
        onOpenLink={open}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Title' })).toBeInTheDocument();
    expect(screen.getByText('pnpm dev').tagName).toBe('CODE');
    expect(screen.getByText('really').tagName).toBe('EM');
    expect(screen.getByText('not').tagName).toBe('DEL');
    expect(screen.getByText('now').tagName).toBe('STRONG');
    expect(screen.getByText('cargo test').closest('pre')).not.toBeNull();
    expect(screen.getByText('nested').closest('ul')?.closest('ol')).not.toBeNull();
    expect(screen.getByText('quoted').closest('blockquote')).not.toBeNull();
    expect(screen.getByRole('table')).toBeInTheDocument();
    expect(screen.getByText(/<script>alert\(1\)<\/script>/)).toBeInTheDocument();
    expect(document.querySelector('script')).toBeNull();
    expect(screen.queryByRole('link', { name: 'bad' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('link', { name: 'the PR' }));
    expect(open).toHaveBeenCalledWith('https://github.com/uvxszi/raygun/pull/1');
  });

  it('links bare addresses without trailing punctuation', () => {
    expect(parseInline('by @someone in https://github.com/uvxszi/raygun/pull/2.')).toEqual([
      { kind: 'text', text: 'by @someone in ' },
      {
        kind: 'link',
        url: 'https://github.com/uvxszi/raygun/pull/2',
        children: [{ kind: 'text', text: 'https://github.com/uvxszi/raygun/pull/2' }],
      },
      { kind: 'text', text: '.' },
    ]);
    expect(parseInline('snake_case_name')).toEqual([{ kind: 'text', text: 'snake_case_name' }]);
    expect(parseMarkdown('- [x] done\n- [ ] todo')).toEqual([
      {
        kind: 'list',
        ordered: false,
        start: 1,
        items: [
          { checked: true, blocks: [{ kind: 'paragraph', text: 'done' }] },
          { checked: false, blocks: [{ kind: 'paragraph', text: 'todo' }] },
        ],
      },
    ]);
  });
});
