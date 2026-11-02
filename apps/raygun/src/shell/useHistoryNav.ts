import { useRouter } from '@tanstack/react-router';
import { useEffect, useState } from 'react';

/**
 * Back and Forward for the app bar (M46.9): whether each has somewhere to go. The router numbers
 * history entries; a new page drops the ones ahead, Back and Forward move between them.
 */
export const useHistoryNav = () => {
  const { history } = useRouter();
  const [place, setPlace] = useState(() => {
    const index = history.location.state.__TSR_index ?? 0;
    return { index, top: index };
  });
  useEffect(
    () =>
      history.subscribe(({ location, action }) => {
        const index = location.state.__TSR_index ?? 0;
        setPlace((previous) => ({
          index,
          top: action.type === 'PUSH' ? index : Math.max(previous.top, index),
        }));
      }),
    [history],
  );
  return {
    canGoBack: place.index > 0,
    canGoForward: place.index < place.top,
    back: () => history.back(),
    forward: () => history.forward(),
  };
};
