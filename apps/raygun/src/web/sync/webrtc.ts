// How the browser reaches a Raygun app (docs/WEB.md): a page served over https can't open a
// socket to a device on the local network, but it can make a WebRTC connection to one. The app
// listens as an ICE-lite peer with a fixed certificate, so everything the browser needs to
// connect is in the app's link code; nothing travels back before the connection is up, and no
// server is involved. Device sync's own bytes (Noise, then its messages) then go over a data
// channel, which authenticates both devices whatever the connection's certificates are.
import { ByteStream, SyncError } from './channel';
import type { Connection } from './engine';
import type { WebInvite } from './protocol';

/** How long the connection may take to come up. */
const CONNECT_TIMEOUT_MS = 15_000;

const colonHex = (hex: string) => (hex.toUpperCase().match(/.{2}/g) ?? []).join(':');

/** The app's side of the connection, as an SDP answer to the browser's offer. */
export const answerFor = (offer: string, invite: WebInvite): string => {
  const mid = /^a=mid:(\S+)/m.exec(offer)?.[1] ?? '0';
  const lines = [
    'v=0',
    `o=- ${Date.now()} 2 IN IP4 127.0.0.1`,
    's=-',
    't=0 0',
    `a=group:BUNDLE ${mid}`,
    'a=ice-lite',
    'm=application 9 UDP/DTLS/SCTP webrtc-datachannel',
    'c=IN IP4 0.0.0.0',
    `a=mid:${mid}`,
    `a=ice-ufrag:${invite.ufrag}`,
    `a=ice-pwd:${invite.pwd}`,
    `a=fingerprint:sha-256 ${colonHex(invite.fingerprint)}`,
    'a=setup:passive',
    'a=sctp-port:5000',
    'a=max-message-size:262144',
    ...invite.hosts.map(
      (host, index) =>
        `a=candidate:${index + 1} 1 udp ${2130706431 - index} ${host} ${invite.port} typ host`,
    ),
    'a=end-of-candidates',
  ];
  return `${lines.join('\r\n')}\r\n`;
};

/** Which of the app's data channels to open: sync's (0), or the back end's (1, `web/remote/`). */
export type ChannelChoice = { id: number; label: string; timeoutMs?: number };

const SYNC_CHANNEL: ChannelChoice = { id: 0, label: 'raygun-sync' };

/** Opens a data channel to the app `invite` names. */
export const connectWebRtc = async (
  invite: WebInvite,
  choice: ChannelChoice = SYNC_CHANNEL,
): Promise<Connection> => {
  if (typeof RTCPeerConnection === 'undefined') {
    throw new SyncError("This browser can't make the connection Raygun needs (WebRTC).");
  }
  const connection = new RTCPeerConnection({ iceServers: [] });
  // Negotiated: both sides know the channel without a message to open it.
  const channel = connection.createDataChannel(choice.label, {
    negotiated: true,
    id: choice.id,
    ordered: true,
  });
  channel.binaryType = 'arraybuffer';
  const close = () => {
    channel.close();
    connection.close();
  };
  try {
    const offer = await connection.createOffer();
    await connection.setLocalDescription(offer);
    await connection.setRemoteDescription({
      type: 'answer',
      sdp: answerFor(offer.sdp ?? '', invite),
    });
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () =>
          reject(
            new SyncError(
              `Couldn't reach ${invite.name || 'the other device'}. Check that it's on the same network with Raygun open, then try again.`,
            ),
          ),
        CONNECT_TIMEOUT_MS,
      );
      channel.onopen = () => {
        clearTimeout(timer);
        resolve();
      };
      connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'failed') {
          clearTimeout(timer);
          reject(
            new SyncError(`Couldn't reach ${invite.name || 'the other device'} on this network.`),
          );
        }
      };
    });
  } catch (error) {
    close();
    throw error;
  }
  const stream = new ByteStream(
    (bytes) => channel.send(bytes as Uint8Array<ArrayBuffer>),
    choice.timeoutMs,
  );
  channel.onmessage = (event: MessageEvent<ArrayBuffer>) => stream.push(new Uint8Array(event.data));
  channel.onclose = () => stream.close();
  connection.onconnectionstatechange = () => {
    if (connection.connectionState === 'failed' || connection.connectionState === 'closed') {
      stream.close();
    }
  };
  return { stream, close };
};
