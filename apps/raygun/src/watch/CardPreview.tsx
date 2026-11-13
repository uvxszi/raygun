import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { FC } from 'react';

import { unwrapResult } from '../lib/commandResult';

/**
 * A Stremio title's preview: the trailer its listing named, else the one its details name (asked
 * only now, once the card is to preview; the title page reads the same answer). Nothing without.
 */
export const StremioPreview: FC<{ kind: string; id: string; trailer?: string | null }> = ({
  kind,
  id,
  trailer,
}) => {
  const meta = useQuery({
    queryKey: ['stremio', 'meta', kind, id],
    queryFn: () => unwrapResult(commands.stremioMeta(kind, id)),
    enabled: !trailer,
    staleTime: 10 * 60 * 1000,
    retry: false,
  });
  const videoId = trailer ?? meta.data?.trailerYtIds[0];
  return videoId ? <CardPreview videoId={videoId} /> : null;
};

/**
 * A title's trailer playing muted over its card (YouTube's embed host page, as the title page's
 * trailer window): it fades in once playing and lets every press through to the card.
 */
export const CardPreview: FC<{ videoId: string }> = ({ videoId }) => {
  const embed = useQuery({
    queryKey: ['youtubeEmbed', false],
    queryFn: () => commands.youtubeEmbedUrl(false),
    staleTime: Infinity,
  });
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [playing, setPlaying] = useState(false);
  const [failed, setFailed] = useState(false);

  const onMessage = useEffectEvent((event: MessageEvent) => {
    const frame = frameRef.current?.contentWindow;
    if (!frame || event.source !== frame) return;
    const data = event.data as { raygunEmbed?: boolean; type?: string; state?: string } | null;
    if (!data?.raygunEmbed) return;
    if (data.type === 'ready') {
      frame.postMessage(
        { raygunCommand: true, type: 'load', videoId, startMs: 0, autoplay: true, volume: 0 },
        '*',
      );
    } else if (data.type === 'state') {
      setPlaying(data.state === 'playing');
    } else if (data.type === 'error') {
      setFailed(true);
    }
  });
  useEffect(() => {
    const listener = (event: MessageEvent) => onMessage(event);
    window.addEventListener('message', listener);
    return () => window.removeEventListener('message', listener);
  }, []);

  if (!embed.data || failed) return null;
  return (
    <span
      aria-hidden
      data-testid="card-preview"
      data-playing={playing ? '' : undefined}
      className="card-preview pointer-events-none absolute inset-0 overflow-hidden bg-black"
    >
      <iframe
        ref={frameRef}
        title=""
        tabIndex={-1}
        src={embed.data}
        allow="autoplay; encrypted-media"
        className="size-full border-0"
      />
    </span>
  );
};
