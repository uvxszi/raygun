import { usePlatform } from '../lib/usePlatform';
import { useRemoteConnected } from '../web/remoteState';

/**
 * Whether what only the Raygun app can do is out of reach here: Raygun on the web (docs/WEB.md)
 * without a linked app answering for it.
 */
export const useNeedsApp = () => {
  const web = usePlatform() === 'web';
  const remote = useRemoteConnected();
  return web && !remote;
};
