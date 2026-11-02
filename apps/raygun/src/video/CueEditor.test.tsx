import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import '../i18n';
import { CueEditor } from './CueEditor';
import { applyCueEdits, editKey, withCueEdit } from './cueEdits';
import type { Cue } from './subtitles';

const CUES: Cue[] = [
  { startMs: 1_000, endMs: 2_500, lines: ['Hello'] },
  { startMs: 3_000, endMs: 4_500, lines: ['How are you?'] },
  { startMs: 5_000, endMs: 6_000, lines: ['Fine'] },
];

describe('Subtitle line editing', () => {
  it('puts edits in place, in time order, and undoes them', () => {
    const edits = withCueEdit([], 3_000, { startMs: 800, endMs: 2_000, text: 'How are\nyou?' });
    expect(applyCueEdits(CUES, edits).map((cue) => [cue.startMs, cue.lines])).toEqual([
      [800, ['How are', 'you?']],
      [1_000, ['Hello']],
      [5_000, ['Fine']],
    ]);
    expect(withCueEdit(edits, 3_000, null)).toEqual([]);
    expect(editKey('stremio:series:tt1:1:2', 'English', null)).toBe(
      'stremio:series:tt1:1:2|English|',
    );
  });

  it('syncs to a line as it is heard and edits a line', async () => {
    const user = userEvent.setup();
    const onDelay = vi.fn();
    const onEdit = vi.fn();
    render(
      <CueEditor
        original={CUES}
        edits={[]}
        timeMs={3_700}
        delayMs={0}
        onDelay={onDelay}
        onEdit={onEdit}
      />,
    );
    const list = screen.getByTestId('cue-editor');
    expect(within(list).getByText('How are you?').closest('li')).toHaveAttribute(
      'aria-current',
      'true',
    );

    // "Fine" is heard now, at 3.7 s, though it starts at 5 s: lines show 1.3 s earlier.
    await user.click(within(list).getByRole('button', { name: 'Heard “Fine” now' }));
    expect(onDelay).toHaveBeenCalledWith(-1_300);

    await user.click(within(list).getByRole('button', { name: 'Edit “Hello”' }));
    const words = within(list).getByRole('textbox', { name: 'Words' });
    await user.clear(words);
    await user.type(words, 'Hi there');
    const end = within(list).getByRole('textbox', { name: 'Ends at' });
    await user.clear(end);
    await user.type(end, '0:02.8');
    await user.click(within(list).getByRole('button', { name: 'Save' }));
    expect(onEdit).toHaveBeenCalledWith(1_000, { startMs: 1_000, endMs: 2_800, text: 'Hi there' });
  });
});
