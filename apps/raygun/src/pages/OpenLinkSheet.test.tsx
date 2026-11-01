import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { OpenLinkSheet } from './OpenLinkSheet';
import { parseYouTubeId, toQueueItem } from './toQueueItem';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

describe('toQueueItem', () => {
  it('streams http(s) URLs and titles them by file name', () => {
    expect(toQueueItem('https://cdn.example.com/music/My%20Song.mp3?sig=1', '')).toMatchObject({
      title: 'My Song.mp3',
      source: {
        kind: 'url',
        url: 'https://cdn.example.com/music/My%20Song.mp3?sig=1',
        headers: {},
      },
    });
  });

  it('adds a Referer header when given', () => {
    expect(toQueueItem('https://host/a.mp3', 'https://host/page')?.source).toEqual({
      kind: 'url',
      url: 'https://host/a.mp3',
      headers: { Referer: 'https://host/page' },
    });
  });

  it('treats anything else as a local file path', () => {
    expect(toQueueItem('C:\\Music\\track.flac', '')).toMatchObject({
      title: 'track.flac',
      source: { kind: 'localFile', path: 'C:\\Music\\track.flac' },
    });
  });

  it('ignores blank input', () => {
    expect(toQueueItem('   ', '')).toBeNull();
  });
});

describe('OpenLinkSheet as audio', () => {
  beforeEach(() => {
    seedBindings();
  });

  it('plays the entered URL immediately and closes', async () => {
    const onClose = vi.fn();
    renderWithProviders(<OpenLinkSheet initialKind="audio" onClose={onClose} />);

    await userEvent.type(screen.getByLabelText('URL or file path'), 'https://host/a.mp3');
    await userEvent.click(screen.getByRole('button', { name: 'Play' }));

    expect(bindingsMock.commands.playbackEnqueue).toHaveBeenCalledWith(
      [expect.objectContaining({ title: 'a.mp3' })],
      true,
    );
    expect(onClose).toHaveBeenCalled();
  });

  it('can add to the queue without interrupting playback', async () => {
    renderWithProviders(<OpenLinkSheet initialKind="audio" onClose={() => {}} />);

    await userEvent.type(screen.getByLabelText('URL or file path'), '/sdcard/Music/b.ogg');
    await userEvent.click(screen.getByRole('button', { name: 'Add to queue' }));

    expect(bindingsMock.commands.playbackEnqueue).toHaveBeenCalledWith(
      [expect.objectContaining({ source: { kind: 'localFile', path: '/sdcard/Music/b.ogg' } })],
      false,
    );
  });
});

describe('parseYouTubeId', () => {
  it.each([
    ['https://www.youtube.com/watch?v=M7lc1UVf-VE', 'M7lc1UVf-VE'],
    ['https://youtu.be/M7lc1UVf-VE?t=42', 'M7lc1UVf-VE'],
    ['https://music.youtube.com/watch?v=M7lc1UVf-VE&list=RD', 'M7lc1UVf-VE'],
    ['https://m.youtube.com/shorts/M7lc1UVf-VE', 'M7lc1UVf-VE'],
    ['M7lc1UVf-VE', 'M7lc1UVf-VE'],
  ])('finds the video in %s', (input, expected) => {
    expect(parseYouTubeId(input)).toBe(expected);
  });

  it.each([
    'https://example.com/watch?v=M7lc1UVf-VE',
    'https://youtu.be/short',
    '/sdcard/Music/a.mp3',
  ])('ignores %s', (input) => {
    expect(parseYouTubeId(input)).toBeNull();
  });

  it('turns YouTube links into video items played by YouTube', () => {
    expect(toQueueItem('https://youtu.be/M7lc1UVf-VE', '')).toMatchObject({
      mediaKind: 'video',
      artworkUrl: 'https://i.ytimg.com/vi/M7lc1UVf-VE/hqdefault.jpg',
      source: { kind: 'youTube', videoId: 'M7lc1UVf-VE' },
    });
  });
});
