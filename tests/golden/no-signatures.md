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
L3    | type      | Config                                                       | (1 lines)
L5    | fn        | run                                                          | (4 lines)
L6    | var       | run > store                                                  | (1 lines)
```

## tests/fixture/src/core.ts
```typescript
L1    | interface | Store                                                        | (3 lines)
L2    | method    | Store > get                                                  | (1 lines)
L5    | class     | MemoryStore                                                  | (7 lines)
L6    | field     | MemoryStore > data                                           | (1 lines)
L8    | method    | MemoryStore > get                                            | (3 lines)
L13   | fn        | makeStore                                                    | (1 lines)
L15   | fn        | internalSeed                                                 | (1 lines)
```

## tests/fixture/src/lib.rs
imports: std::collections::HashMap
```rust
L3    | enum      | Mode                                                         | (4 lines)
L8    | trait     | Render                                                       | (3 lines)
L9    | method    | Render > render                                              | (1 lines)
L13   | method    | Mode > render                                                | (3 lines)
L18   | const     | MAX                                                          | (1 lines)
L20   | fn        | hidden                                                       | (4 lines)
L21   | var       | hidden > size                                                | (1 lines)
```

## tests/fixture/src/util.py
imports: json
```python
L4    | class     | Widget                                                       | (8 lines)
L6    | method    | Widget > label                                               | (2 lines)
L9    | method    | Widget > render                                              | (3 lines)
L10   | var       | render > payload                                             | (1 lines)
L14   | fn        | _private_helper                                              | (2 lines)
```

