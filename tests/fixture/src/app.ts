import { makeStore } from './core';

export type Config = { verbose: boolean };

export const run = (config: Config): void => {
  const store = makeStore();
  store.get('seed');
};
