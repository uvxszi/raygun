import type { FC } from 'react';

export const PageHeader: FC<{ title: string }> = ({ title }) => (
  <h1 className="text-2xl font-bold tracking-tight lg:text-3xl">{title}</h1>
);
