# Tomo

> A friendly, TOML-native API client. Lightweight, offline, git-first.

**Tomo** (TOM(L) · "tomo" = volume/book in Portuguese · 友 "friend" in Japanese) is a minimalist desktop API client built with Tauri 2 + Rust. Collections are plain folders of readable TOML files — no accounts, no cloud, no proprietary database.

## Status

🚧 Under active development — v1 in progress.

## Stack

Tauri 2 · Rust (reqwest, toml_edit, boa) · React 19 · TypeScript · Tailwind CSS v4 · CodeMirror 6 · Zustand

## Development

```sh
# System prerequisites (Linux)
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev

pnpm install
pnpm tauri dev      # desktop app
pnpm dev            # frontend only (browser, mocked transport)

cargo test -p tomo-core   # core test suite
pnpm test                 # frontend test suite
```

## License

[MIT](LICENSE) © Daniel Freitas
