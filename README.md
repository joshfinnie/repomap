# repomap

`repomap` is a lightweight, polyglot CLI tool built in Rust that generates a structured Markdown "map" of your repository.
It is designed specifically to provide high-density context to LLMs (like Gemini, Claude, or ChatGPT) without exhausting their context window with boilerplate code.

Unlike simple file-tree tools, `repomap` uses Tree-sitter to parse your code and extract meaningful symbols (functions, structs, classes, and methods) while maintaining their logical hierarchy.

## Features

- **Polyglot Support**: Deep parsing for Rust, Python, Go, TypeScript, TSX, JavaScript, and Markdown.
- **Hierarchical Breadcrumbs**: Identifies methods within their parents
- **AI-Optimized**: Estimates token counts and generates clean Markdown blocks ready for copy-pasting.
- **Git-Aware**: Automatically respects .gitignore and hidden files using the ignore crate.
- **Summary Tables**: Optional high-level overview of file density and symbol counts.
- **Depth Control**: Limit traversal depth for a "big picture" view of large monorepos.

## Install

```bash
git clone https://github.com/joshfinnie/repomap
cd repomap
cargo install --path .
```

## Usage

### Basic Map

Generate a map of the current directory and print to stout:

```bash
repomap .
```

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

### Save to a file

```bash
repomap -s . -o repomap.md
```

## Supported Languages & Patterns

| Language         | Captured Symbols                       |
| ---------------- | -------------------------------------- |
| Rust             | Structs, Functions, and impl methods   |
| TypeScript / TSX | Classes, Interfaces, and Methods       |
| Python           | Classes and Function definitions       |
| Go               | Types, Functions, and Method receivers |
| Markdown         | H1, H2, and H3 Headers                 |

## Why `repomap`?

When working with LLMs, the "Context Window" is your most valuable resource.
Pasting entire files often includes 80% boilerplate (imports, CSS-in-JS, repetitive logic) and only 20% intent.

`repomap` flips that ratio.
It provides the AI with the signatures and structure, allowing it to understand where logic lives and how it's organized, so you only have to paste the specific implementation details when they matter.

## License

Copyright 2026 Josh Finnie <josh@jfin.us>

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
