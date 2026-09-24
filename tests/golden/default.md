# Repository Map
**Root:** `tests/fixture`
**Files:** 5

---

## tests/fixture/README.md
```markdown
L1    | h1        | Fixture                                                      | (1 lines)
L3    | h2        |   Purpose                                                    | (1 lines)
L5    | h3        |     Details                                                  | (1 lines)
```

## tests/fixture/src/app.ts
imports: ./core
```typescript
L3    | type      | export type Config = { verbose: boolean }                    | (1 lines)
L5    | fn        | export const run = (config: Config): void                    | (4 lines)
```

## tests/fixture/src/core.ts
```typescript
L1    | interface | export interface Store                                       | (3 lines)
L2    | method    | Store > get(id: string): string | undefined                  | (1 lines)
L5    | class     | export class MemoryStore implements Store                    | (7 lines)
L6    | field     | MemoryStore > private data: Map<string, string> = new Map()  | (1 lines)
L8    | method    | MemoryStore > get(id: string): string | undefined            | (3 lines)
L13   | fn        | export const makeStore = (): Store                           | (1 lines)
L15   | fn        | const internalSeed = ()                                      | (1 lines)
```

## tests/fixture/src/lib.rs
imports: std::collections::HashMap
```rust
L3    | enum      | pub enum Mode                                                | (4 lines)
L8    | trait     | pub trait Render                                             | (3 lines)
L9    | method    | Render > fn render(&self) -> String                          | (1 lines)
L13   | method    | Mode > fn render(&self) -> String                            | (3 lines)
L18   | const     | pub const MAX: u32 = 10                                      | (1 lines)
L20   | fn        | fn hidden(cache: &HashMap<u32, String>) -> usize             | (3 lines)
```

## tests/fixture/src/util.py
imports: json
```python
L4    | class     | class Widget                                                 | (7 lines)
L6    | method    | Widget > def label(self)                                     | (2 lines)
L9    | method    | Widget > def render(self, indent=0)                          | (2 lines)
L13   | fn        | def _private_helper()                                        | (2 lines)
```

