# EasySearch

## Acknowledgments / 代码参考

The MFT reading, USN journal, and pipe security modules (`easysearch-mft`, `easysearch-security`, `uffs-polars`, `uffs-text`) are derived from the [UltraFastFileSearch](https://github.com/skyllc-ai/UltraFastFileSearch) project by SKY, LLC (MPL-2.0 license). These modules provide the low-level NTFS MFT parsing, USN change journal integration, Polars DataFrame facade, and Windows named-pipe DACL helpers that EasySearch builds upon.

Original repository: https://github.com/skyllc-ai/UltraFastFileSearch  
Original license: MPL-2.0  
Original author: Robert Nio / SKY, LLC

A fast Windows file search engine that indexes NTFS volumes via MFT parsing and keeps the index live through USN journal polling.

## Architecture

- **easysearch-core** — In-memory filename/path index, search scoring, cache persistence.
- **easysearch** — Daemon binary (`easysearch.exe`) that builds indexes, polls the USN journal, and serves search/enumerate queries over a named pipe (NDJSON protocol).
- **easysearch-mft** — NTFS MFT reader and USN journal interface.
- **easysearch-security** — Named-pipe DACL for per-user access control.

## Building

```powershell
cargo build --release -p easysearch
```

Requires:
- Rust nightly (pinned in `rust-toolchain.toml`)
- `sccache` installed
- Administrator privileges at runtime for MFT/USN access

## Environment Variables

| Variable | Description |
|----------|-------------|
| `EASYSEARCH_PIPE` | Override named-pipe path |
| `EASYSEARCH_DRIVES` | Comma-separated drive letters to index (default: `C`) |
| `EASYSEARCH_CACHE_DIR` | Cache directory for `.flowcache` files |

## License

MIT — Copyright (c) 2025-2026 LIJIALU
