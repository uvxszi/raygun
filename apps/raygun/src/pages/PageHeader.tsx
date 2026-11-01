import type { ComponentProps, FC } from 'react';

import { TopBar } from '../shell/TopBar';

/** A screen's title: the top app bar (kept for screens that need nothing more than a title). */
export const PageHeader: FC<ComponentProps<typeof TopBar>> = (props) => <TopBar {...props} />;
