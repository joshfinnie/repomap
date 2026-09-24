export interface Store {
  get(id: string): string | undefined;
}

export class MemoryStore implements Store {
  private data: Map<string, string> = new Map();

  get(id: string): string | undefined {
    return this.data.get(id);
  }
}

export const makeStore = (): Store => new MemoryStore();

const internalSeed = () => 42;
