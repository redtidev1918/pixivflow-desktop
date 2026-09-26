# Development

## Prerequisites

Required:

- **Node.js** >= 22
- **Rust** (stable toolchain)
- **Tauri CLI**

## Install

```bash
npm install
```

## Development

```bash
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

> **F0 note:** the commands above are listed for reference once the Tauri shell
> lands in **Phase 1 (F1)**. This stage is **F0 — Foundation** only: the
> repository contains no Tauri project, no `package.json`, no Rust source, and
> nothing to run or build yet. Do **not** run `cargo init`, `npm install` or
> other scaffolding now.

## Constraints

- No Rust / business logic is introduced at F0.
- No bundling of `backend` / `webui` assets at F0 (that belongs to Phase 2).