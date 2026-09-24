# repomap

[![Version](https://img.shields.io/github/v/release/joshfinnie/repomap)](https://github.com/joshfinnie/repomap/releases)
[![Release](https://github.com/joshfinnie/repomap/actions/workflows/release.yml/badge.svg)](https://github.com/joshfinnie/repomap/actions/workflows/release.yml)
[![CI](https://github.com/joshfinnie/repomap/actions/workflows/ci.yml/badge.svg)](https://github.com/joshfinnie/repomap/actions/workflows/ci.yml)

`repomap` is a lightweight, polyglot CLI tool built in Rust that generates a structured Markdown "map" of your repository.
It is designed specifically to provide high-density context to LLMs (like Gemini, Claude, or ChatGPT) without exhausting their context window with boilerplate code.

Unlike simple file-tree tools, `repomap` uses Tree-sitter to parse your code and extract meaningful symbols (functions, structs, classes, and methods) while maintaining their logical hierarchy.

## Features

- **Polyglot Support**: Deep parsing for Rust, Python, Go, TypeScript, TSX, JavaScript, Java, C, C++, C#, Ruby, PHP, Swift, Kotlin, and Markdown.
- **Full Signatures**: Emits the whole declaration, parameters and return type included, so a model can call your code rather than just guess that it exists.
- **Import Extraction**: Lists imports/dependencies for each file to help understand module relationships.
- **Relevance Ranking**: Scores every file by its centrality in the import graph, so `--max-tokens` spends a fixed budget on the files the rest of the repo actually depends on.
- **Hierarchical Breadcrumbs**: Identifies methods within their parents (e.g., `ClassName > method`).
- **JSON Output**: `--format json` for scripts, hooks, and MCP servers that want the symbol table rather than the prose.
- **Git-Aware**: Respects .gitignore and hidden files, and `--since` narrows the map to what changed against a ref.
- **CLAUDE.md Integration**: Smart append/update to your existing CLAUDE.md files.
- **Summary Tables**: Optional high-level overview of file density and symbol counts.
- **Depth Control**: Limit traversal depth for a "big picture" view of large monorepos.
- **Nested Bindings**: Captures bindings inside function bodies, each labelled with the function it lives in, or drop them with `--no-locals`.
- **Minimal Mode**: Strip imports, signatures, line numbers, and code blocks down to just symbol names for maximum density.

## Installation

### From Release (Recommended)

Download the pre-compiled binary for your system from the [Releases](https://github.com/joshfinnie/repomap/releases) page.

1. Download the `.tar.gz` for your OS
2. Extract the binary: `tar -xvf repomap-*.tar.gz`
3. Move it to your path: `mv repomap /usr/local/bin`

### From Source

If you have the Rust toolchain installed:

```bash
git clone https://github.com/joshfinnie/repomap
cd repomap
cargo install --path .
```

## Usage

### Basic Map

Generate a map of the current directory and print to stdout:

```bash
repomap .
```

Each symbol comes out with its full signature:

```
L44   | fn        | pub fn analyze_file(path: &Path, lang: Language) -> Result<FileEntry> | (38 lines)
L39   | method    | FileEntry > pub fn is_empty(&self) -> bool                   | (3 lines)
```

Pass `--no-signatures` for bare names if you want the older, denser shape.

### Locals

Bindings inside a function body are captured too, each carrying the nearest enclosing declaration as its breadcrumb:

```
L5    | fn        | export const run = (config: Config): void                    | (4 lines)
L6    | var       | run > const store = makeStore()                              | (1 lines)
L20   | fn        | fn hidden(cache: &HashMap<u32, String>) -> usize             | (4 lines)
L21   | var       | hidden > let size = cache.len()                              | (1 lines)
```

This covers `const`/`let`/`var` in JavaScript and TypeScript, `let` in Rust, and plain assignment in Python. A binding whose value is a function or closure reads as `fn` rather than `var`. A `const` inside a function body is a local; the same declaration at the top of the file is not.

It costs real tokens. Measured on three repositories:

| Repository | With locals | `--no-locals` |
| ---------- | ----------- | ------------- |
| repomap's own `src` (Rust) | ~12.9k | ~5.0k |
| a Next.js/TypeScript app | ~20.2k | ~13.3k |
| another TypeScript app | ~35.9k | ~20.1k |

Rust takes the biggest hit, since `let` carries work that other languages put in expressions. When you want only a file's outward surface:

```bash
repomap --no-locals .
```

Nearest scope wins, so a binding inside a closure names the closure rather than the outermost function. `for` loop counters are skipped. `let a, b` reports two symbols rather than one line twice. A name bound more than once in the same scope (Python reassignment, Rust shadowing) is reported where it first appears.

### Fit a Token Budget

This is the flag to reach for on a repo too big to map whole.
`repomap` ranks every file by how central it is to the import graph (a file that half the repo imports scores far above a leaf), then keeps the highest-ranked files that fit:

```bash
repomap --max-tokens 4000 .
```

The map says how many files it left out, so you know the view is partial.

To rank by proximity to whatever you are actually working on instead of by global importance:

```bash
repomap --max-tokens 4000 --focus src/parser.rs .
```

### Only What Changed

Useful for handing a model context on a branch or a PR without the rest of the repo:

```bash
repomap --since main .
repomap --since HEAD~3 .
```

Modified tracked files and new untracked files both count as changed.

### JSON

For hooks, scripts, and anything that wants to consume the symbol table directly:

```bash
repomap -f json .
```

Every symbol carries its name, kind, parent, line range, signature, and whether it is exported and whether it is local.
Files carry their import list and their ranking score.

### With Summary and Table of Contents

Great for a high-level overview of project scale:

```bash
repomap -s .
```

### Limit Traversal Depth

Useful for large projects where you only want to see the top-level architecture:

```bash
repomap --depth 2 .
```

### Minimal Output

For very large repos where even the full map is too big for your context window, `--minimal` drops imports, signatures, line numbers, and code blocks and prints just symbol names and hierarchy, useful as a first-pass overview before drilling into specific files:

```bash
repomap -m .
```

### Save to a file

```bash
# Defaults to repomap.md
repomap -o

# Or specify a custom path
repomap -o my-map.md

# With summary table
repomap -s -o
```

Note: `repomap.md` and `CLAUDE.md` are automatically excluded from processing to prevent self-referential loops.

### CLAUDE.md Integration

Use the `--claude` flag to output directly to your project's `CLAUDE.md` with smart update behavior:

```bash
# Creates or appends to CLAUDE.md
repomap --claude

# Subsequent runs update the map section in place
repomap --claude

# Map a specific subdirectory
repomap --claude src/api

# Custom output path with CLAUDE.md formatting
repomap --claude -o docs/CLAUDE.md
```

The `--claude` flag wraps the output in a collapsible `<details>` block with `<!-- REPOMAP START -->` and `<!-- REPOMAP END -->` markers. Running the command again will replace just the map section while preserving the rest of your `CLAUDE.md` content.

## Supported Languages & Patterns

| Language         | Captured Symbols                                                        | Imports |
| ---------------- | ----------------------------------------------------------------------- | ------- |
| Rust             | Functions, structs, enums, unions, traits, type aliases, consts, statics, modules, macros, impl and trait methods, `let` bindings | `use` statements |
| TypeScript / TSX | Classes, interfaces, type aliases, enums, namespaces, functions, methods, and `const`/`let` bindings at any depth | `import` / `export from` / dynamic `import()` |
| JavaScript       | Classes, functions, generators, methods, class fields, and `const`/`let`/`var` bindings at any depth | `import` / `export from` / dynamic `import()` |
| Python           | Classes, functions, methods (decorated ones included), class attributes, local assignments, module-level `CONSTANTS` | `import` / `from ... import` |
| Go               | Functions, types, type aliases, consts, vars, method receivers, interface methods | `import` specs |
| Java             | Classes, interfaces, enums, records, annotations, methods, constructors  | `import` declarations |
| C                | Functions, prototypes, structs, enums, unions, typedefs                 | `#include` |
| C++              | Everything C captures, plus classes, namespaces, and member functions   | `#include` |
| C#               | Classes, interfaces, structs, enums, records, delegates, namespaces, methods, properties, constructors | `using` directives |
| Ruby             | Classes, modules, methods, singleton methods                            | `require` / `require_relative` / `load` |
| PHP              | Classes, interfaces, traits, enums, functions, methods                  | `use` declarations |
| Swift            | Classes, structs, enums, protocols, functions, type aliases, methods    | `import` declarations |
| Kotlin           | Classes, objects, functions, properties, member functions               | `import` declarations |
| Markdown         | Headings, all levels, indented by depth                                 | - |

File extensions recognized include `.rs`, `.py`, `.pyi`, `.go`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`, `.tsx`, `.java`, `.c`, `.h`, `.cc`, `.cpp`, `.cxx`, `.hpp`, `.hh`, `.hxx`, `.cs`, `.rb`, `.rake`, `.php`, `.swift`, `.kt`, `.kts`, `.md`, and `.markdown`.

## Why `repomap`?

When working with LLMs, the "Context Window" is your most valuable resource.
Pasting entire files often includes 80% boilerplate (imports, CSS-in-JS, repetitive logic) and only 20% intent.

`repomap` flips that ratio.
It provides the AI with the signatures and structure, allowing it to understand where logic lives and how it's organized, so you only have to paste the specific implementation details when they matter.

## Development

**Running Tests**

```bash
cargo test
```

Unit tests cover each language's Tree-sitter queries in isolation, including one test that compiles every query against its own grammar so a typo in a node name fails loudly instead of silently dropping symbols.

The integration tests in `tests/cli.rs` run the real binary against the fixture repo in `tests/fixture` and diff whole outputs against the golden files in `tests/golden`. After an intentional output change:

```bash
REPOMAP_UPDATE_GOLDEN=1 cargo test --test cli
```

Read the resulting diff before committing it.

**Binary Safety** - The tool automatically detects and skips binary files to prevent parser crashes and token waste.

## License

Copyright 2026 Josh Finnie <josh@jfin.us>

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
