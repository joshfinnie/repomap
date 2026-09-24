# Repository Map
**Root:** `tests/fixture`
**Files:** 5

## Summary
| File | Symbols | Lines |
| :--- | :--- | :--- |
| `tests/fixture/README.md` | 3 | 5 |
| `tests/fixture/src/app.ts` | 3 | 8 |
| `tests/fixture/src/core.ts` | 7 | 15 |
| `tests/fixture/src/lib.rs` | 7 | 23 |
| `tests/fixture/src/util.py` | 5 | 15 |

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
L6    | var       | run > const store = makeStore()                              | (1 lines)
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
L20   | fn        | fn hidden(cache: &HashMap<u32, String>) -> usize             | (4 lines)
L21   | var       | hidden > let size = cache.len()                              | (1 lines)
```

## tests/fixture/src/util.py
imports: json
```python
L4    | class     | class Widget                                                 | (8 lines)
L6    | method    | Widget > def label(self)                                     | (2 lines)
L9    | method    | Widget > def render(self, indent=0)                          | (3 lines)
L10   | var       | render > payload = {"label": self.label}                     | (1 lines)
L14   | fn        | def _private_helper()                                        | (2 lines)
```

