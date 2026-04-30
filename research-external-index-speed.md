# Research: speeding up local developer-tool file indexing pipelines

## Summary
Current authoritative evidence points to the same architecture used by mature code-search/editor systems: do one optimized cold crawl, then keep a persistent index current with filesystem watchers/incremental queries; batch writes into SQLite/FTS/vector stores; and avoid re-reading/re-parsing unchanged content. Highest-confidence wins for this repo are: aggressive path pruning and parallel walking, change-detection metadata plus fast content hashes, transaction-batched FTS/vector writes, incremental parse reuse, and index compaction/maintenance.

## Findings
1. **Do less work before doing faster work: prune during traversal and respect ignore/exclude rules.** `walkdir` emphasizes efficient recursive traversal with controls for max open file descriptors and pruning directory trees, while `jwalk` adds parallel traversal, streamed sorted output, and custom filter/skip hooks. For codebase indexing, early exclusion of `.git`, `node_modules`, build outputs, binaries, and ignored paths prevents downstream hashing/parsing/index writes entirely. **Confidence: high.** [walkdir](https://github.com/BurntSushi/walkdir) [jwalk](https://docs.rs/jwalk/latest/jwalk/index.html)

2. **Parallelize the cold crawl, but cap filesystem pressure.** `jwalk` is explicitly parallel via Rayon; ripgrep-style systems also rely on multithreaded traversal/search and low-allocation I/O. Practical implication: use a bounded worker pool for stat/read/hash/parse stages, not unbounded tasks, and separate I/O-bound walking from CPU-bound parsing/embedding. **Confidence: high.** [jwalk](https://docs.rs/jwalk/latest/jwalk/index.html) [ripgrep performance guide](https://entropicdrift.com/showcase/ripgrep/performance/)

3. **Use persistent watcher state for incremental indexing instead of rescanning on every query/start.** Watchman’s file queries are built around maintained indexes over the watched tree, including `since` queries for files modified since a clock value, so clients do not crawl the filesystem in real time. VS Code’s watcher internals similarly focus on scoped/correlated watch requests to reduce global event processing. **Confidence: high.** [Watchman file queries](https://facebook.github.io/watchman/docs/file-query) [VS Code watcher internals](https://github.com/microsoft/vscode/wiki/File-Watcher-Internals)

4. **Incremental code-search indexes commonly use shards/deltas and compaction.** Sourcegraph’s Zoekt stores index shards on disk, flushes by input-size thresholds, uses shard merging to improve query behavior for many small/stale repos, and has delta-shard work to reindex changed files instead of rebuilding whole repos. For a local indexer, this supports a design with per-file/per-batch segments plus periodic merge/compaction. **Confidence: medium-high.** [Sourcegraph shard merging](https://sourcegraph.com/blog/tackling-the-long-tail-of-tiny-repos-with-shard-merging) [Zoekt delta shard PR](https://github.com/sourcegraph/zoekt/pull/310)

5. **Batch SQLite writes inside explicit transactions and use WAL when reads must continue during indexing.** SQLite documents that all database writes happen in transactions, that implicit one-statement transactions commit after each statement, and that WAL allows readers and a writer to proceed concurrently while writes append to the WAL. This makes per-file commits a likely bottleneck; group many file updates/deletes/inserts per transaction and consider WAL for interactive search during indexing. **Confidence: high.** [SQLite transactions](https://www.sqlite.org/lang_transaction.html) [SQLite WAL](https://www.sqlite.org/wal.html)

6. **For SQLite FTS5, choose table shape deliberately and maintain the index.** FTS5 supports external-content and contentless tables to avoid duplicating stored text, the trigram tokenizer for substring/LIKE-style search, and `merge`/`optimize` commands for index maintenance. Implication: store canonical file metadata/content separately, keep FTS rows lean, update by rowid, and schedule FTS maintenance after large batches rather than during every file update. **Confidence: high.** [SQLite FTS5](https://www.sqlite.org/fts5.html)

7. **Run SQLite statistics maintenance with modern `PRAGMA optimize`.** SQLite recommends `PRAGMA optimize` rather than direct `ANALYZE` in modern versions, including periodic use for long-lived connections and after schema/index changes; since 3.46.0 it automatically applies a temporary analysis limit so it finishes quickly on large DBs. **Confidence: high.** [SQLite ANALYZE / PRAGMA optimize](https://sqlite.org/lang_analyze.html)

8. **Use fast content fingerprints to skip redundant parsing/indexing; choose hash by risk.** BLAKE3 is designed for SIMD and multithreaded Merkle-tree hashing; xxHash/XXH3 advertises very high non-cryptographic throughput in official benchmarks. For local indexing, combine cheap metadata `(mtime, size, inode/path)` with a fast content hash when metadata changes or correctness matters; use cryptographic BLAKE3 if collision risk matters for cache correctness, or XXH3 for fastest non-crypto checks. **Confidence: high.** [BLAKE3 official README](https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md) [xxHash benchmarks](https://xxhash.com/doc/v0.8.3/index.html)

9. **Avoid redundant parsing with incremental syntax trees.** Tree-sitter is explicitly designed for editor-like incremental parsing: edit the old tree, reparse with the old tree, and unchanged parts are reused; APIs also expose changed ranges. If the repo parses files for symbols/chunks, cache parse trees or derived syntax artifacts and only recompute changed ranges/files. **Confidence: high.** [Tree-sitter advanced parsing](https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html)

10. **Vector indexes need special care for write-heavy local workflows.** SQLite vector extensions are convenient, but sqlite-vss maintainers note that its Faiss-in-shadow-table strategy rewrites the whole Faiss index on commit, making frequent small updates expensive. LanceDB documents that new data may need reindexing and may be searched with a mix of indexed plus flat search until reindexing runs. Implication: batch embedding/vector inserts, debounce vector index rebuilds, or use a store that supports incremental writes efficiently. **Confidence: medium.** [sqlite-vss write-heavy issue](https://github.com/asg017/sqlite-vss/issues/30) [LanceDB reindexing](https://docs.lancedb.com/indexing/reindexing)

11. **Measure indexing bottlenecks by phase, not just total time.** Meilisearch exposes batch `progressTrace` to show where asynchronous indexing time is spent; Lance FTS parallelization work identifies tokenization, occurrence collection, lock duration, string cloning, and memory layout as concrete bottlenecks. A local indexer should log timings/counters for walk, stat, read, hash, parse, embed, SQL write, FTS optimize, and vector build. **Confidence: medium-high.** [Meilisearch batch statistics](https://meilisearch.com/docs/learn/indexing/optimize_indexing_performance) [Lance FTS parallelization PR](https://github.com/lancedb/lance/pull/2807)

## Implications for this repo
- Build a staged pipeline: bounded parallel walk/stat -> filter/ignore -> read/hash -> parse/chunk/embed -> batched DB writes -> deferred FTS/vector maintenance.
- Persist per-file metadata: path, inode/file id where available, size, mtime/ctime, content hash, parser version, embedder version, index schema version, and last indexed commit/watch clock.
- On startup, prefer watcher/watchman-style `since` reconciliation plus targeted stat checks over a full rescan; fall back to cold crawl if watcher state is invalid.
- Batch updates/deletes in explicit SQLite transactions; enable WAL if searches should run while indexing; run `PRAGMA optimize` and FTS `optimize`/merge after large batches or idle periods.
- Cache parse/chunk outputs and embeddings by `(content hash, tool version)` to avoid re-parsing/re-embedding identical content, including renames.
- Treat vector indexing as eventually consistent: insert embeddings in batches and rebuild/compact vector structures on debounce/idle schedules.

## Sources
- Kept: Watchman File Queries (https://facebook.github.io/watchman/docs/file-query) — primary evidence for maintained tree indexes and `since` incremental queries.
- Kept: VS Code File Watcher Internals (https://github.com/microsoft/vscode/wiki/File-Watcher-Internals) — ecosystem guidance on scoped watcher events and reducing event-processing overhead.
- Kept: SQLite FTS5 Extension (https://www.sqlite.org/fts5.html) — primary FTS table, trigram, external-content, merge, and optimize documentation.
- Kept: SQLite WAL (https://www.sqlite.org/wal.html) and Transactions (https://www.sqlite.org/lang_transaction.html) — primary evidence for batching and read/write concurrency constraints.
- Kept: Tree-sitter Advanced Parsing (https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html) — primary incremental parsing guidance.
- Kept: jwalk docs and walkdir repo — primary Rust filesystem walking APIs relevant to parallel/pruned traversal.
- Kept: BLAKE3 and xxHash official docs — primary hashing design/benchmark evidence.
- Kept: Sourcegraph Zoekt shard-merging blog and delta-shard PR — direct code-search index architecture evidence.
- Kept: LanceDB reindexing, sqlite-vss issue, Meilisearch batch statistics — practical evidence for vector/index maintenance and instrumentation.
- Dropped: SEO-style blog posts on generic directory walking — less authoritative than crate docs and mature tool docs.
- Dropped: Unmaintained/demo code-search repos claiming speedups — useful anecdotes but not authoritative enough for design decisions.
- Dropped: SQLite forum troubleshooting threads — too narrow compared with official FTS5 documentation.

## Gaps
- No repo-specific benchmark was run, so expected speedups are directional, not quantified.
- Filesystem watcher behavior varies by OS and network filesystem; validate on macOS FSEvents, Linux inotify/fanotify, Windows ReadDirectoryChangesW, and remote/WSL cases if supported.
- Vector-store guidance is evolving quickly; benchmark the exact extension/store and embedding batch sizes before committing to sqlite-vss/sqlite-vec/LanceDB/HNSW choices.
