# YakTool

YakTool — Tell your computer what to do.

YakTool 0.1 is a Linux command-line program for a narrow set of local file operations. Deterministic requests execute locally; unresolved language may use a local Ollama semantic model. The model has no filesystem or execution authority, and all actions remain behind YakTool's trusted policy and confirmation pipeline.

The working vertical slice is interpretation → typed intent → read-only resolution → static policy → frozen plan → preview → explicit confirmation → no-replace rename → filesystem verification → SQLite journal → explicit undo.

## Build and run

Rust 1.85.1 or newer, a C compiler (for bundled SQLite), Linux 5.6+ with `openat2`, and filesystem support for `renameat2(RENAME_NOREPLACE)` are required. Dependencies are pinned by `Cargo.lock`. Cargo may access the network during development; YakTool does not do so at runtime.

```sh
cargo build --release
./target/release/yaktool --help
./target/release/yaktool doctor
./target/release/yaktool "show Downloads"
./target/release/yaktool "how many directories in home?"
./target/release/yaktool "count files in Downloads"
./target/release/yaktool "find PDFs modified this week in Documents"
./target/release/yaktool "find files larger than 500 MB in Downloads"
./target/release/yaktool "move PNG files older than 30 days from Downloads to Archive"
./target/release/yaktool history
./target/release/yaktool show-plan <transaction-ID>
./target/release/yaktool undo
```

There is no installation step required; the executable is `target/release/yaktool`. Move examples require existing source and destination directories. YakTool does not create `Archive` for you. Do not try examples on valuable files before reviewing the safety boundary below.

## Supported language

Each invocation takes one quoted request; there is no conversational session.

| Capability | Examples |
| --- | --- |
| list | `show Downloads`, `list Downloads`, `show me Downloads`, `show me the files in Downloads` |
| search | `find PDFs in Downloads`, `find PDF files in downloads`, `find PNG files in Pictures` |
| date search | `find files older than 30 days in Downloads`, `find PDFs modified this week in Documents`, `find text files modified today in Documents` |
| find_large | `find files larger than 500 MB in Downloads` |
| count | `how many directories in home?`, `count files in Downloads` |
| move | `move PNG files older than 30 days from Downloads to Archive`, `move PDFs from Downloads to Documents` |

The grammar ignores case, repeated whitespace, and trailing periods. Categories are PDF/PDFs (`.pdf`), PNG (`.png`), JPG/JPEG (`.jpg`, `.jpeg`), text (`.txt`), and `files`. Extension comparisons ignore ASCII case. Filters support one size or date qualifier. Fractional sizes, fuzzy sizes, recursion, multiple combined qualifiers, literal paths in natural language, and other English are unsupported. `move my stuff` requests clarification; `delete junk`, `run ls -la`, and `install firefox` are unsupported. None infer operations.

Aliases are `home`, `desktop`, `documents`, `downloads`, `pictures`, and `archive`. The root is the current account's home from the OS account database, canonicalized once; `$HOME` does not establish authority. XDG `user-dirs.dirs` assignments for Desktop/Documents/Downloads/Pictures are parsed as data. Fallbacks use capitalized directories in the account home; Archive always means `~/Archive`. An XDG alias outside the approved root or through a symlink is refused. Relative `XDG_CONFIG_HOME` and `XDG_DATA_HOME` values use the standard fallback; a data directory outside the root is refused.

All requests are non-recursive and exclude entries whose name starts with `.`. Listing displays files, symlinks, and other types without traversing symlinks. Search may display matching symlinks; it uses link metadata, not target metadata. A matching symlink or special file denies a move rather than silently skipping a suspicious source. Only regular files move.

## Exact size and date rules

KB/MB/GB are 1,000 / 1,000,000 / 1,000,000,000 bytes. KiB/MiB/GiB are 1,024 / 1,048,576 / 1,073,741,824 bytes. `larger than` means strictly greater than the specified size. Integer quantities only; overflow is refused.

`older than N days` means strictly before the captured current time minus N × 86,400 seconds. `newer than` means strictly after that instant. `modified today` includes local midnight through the captured current instant; `modified this week` starts at local Monday midnight. Future timestamps are excluded from today/this-week queries. An ambiguous or nonexistent local midnight is refused. Local timezone determines calendar boundaries; plans store exact Unix nanoseconds, never unresolved date phrases.

Moves allow 0–100 files and at most 1 GiB (1,073,741,824 bytes), inclusive. Zero matches produces a no-op preview. More than 100 matches or more than 1 GiB denies the entire plan; no truncation. Limits live in `src/policy.rs`.

## Architecture and plans

“Rules interpret. Tools act. Policies decide. Verification proves.”

One Rust package contains `intent`, `interpret`, `resolve`, `policy`, `plan`, `filesystem`, `execute`, `journal`, `render`, and `cli` modules. The small library target makes the trusted core testable; it is not a workspace or a plugin system.

`Interpreter::interpret` returns a typed, versioned `Intent`. The resolver only reads the filesystem. Its `FileQuery` has exact time and size bounds. Policy must accept the resolved objects before a plan is offered. The executor accepts a `ConfirmedPlan`, never a request or query, and never re-runs a search.

Plans contain exact source/destination paths, source snapshots (device, inode, mode, size, nanosecond mtime and ctime), parent directory identities, and the approved root identity. Query fields are preview evidence only; operations are the sole execution input. Parent snapshots compare identity/mode rather than directory mtime, which the program's own renames change.

Linux paths serialize as arrays of raw bytes. Operations sort by raw source bytes. Canonical serialization is compact `serde_json` output with Rust struct field order and no maps; SHA-256 covers the complete document. The hash is stored separately in SQLite, avoiding a self-reference. This is a fixed V1 encoding, not a general JSON canonicalization standard. Historical loading requires matching schema version, canonical bytes, and hash. Hashes detect representation changes, not malicious database rewriting or changed filesystem objects.

The preview shows up to ten exact operations and the full hash. While the confirmation prompt is open, a second `show-plan <ID>` can display every stored path and canonical JSON. Only `y` or `yes` (case-insensitive, trimmed) confirms. Empty input, EOF, or other responses cancel. There is no bypass flag. Undo also previews and requires confirmation.

## Filesystem safety and its boundary

- No arbitrary shell execution. No process launcher exists in the runtime.
- No root required. Root and mismatched real/effective credentials are refused.
- No overwrite. Both move and undo use kernel `RENAME_NOREPLACE`; there is no check-then-plain-rename fallback.
- No delete. No copy fallback. No cross-filesystem moves.
- No symlink traversal for mutation. Directory lookups use `openat2` with `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, and `RESOLVE_NO_XDEV` from a pinned root descriptor. `NO_SYMLINKS` implies `NO_MAGICLINKS`. Final entries are inspected with `O_PATH|O_NOFOLLOW`; any source found to be a symlink or other nonregular file is rejected.
- Parent traversal and paths outside the root are rejected by path components and descriptor resolution, not string-prefix comparisons. Bind mounts and nested mounts are also refused, even when device numbers match.
- The whole plan is revalidated after confirmation, before any move. Each operation then holds both parent directory FDs through validation, rename, and verification. Immediately before rename, the source entry is inspected relative to that same source-parent FD and compared with the frozen device, inode, mode/type, size, mtime, and ctime. Its object FD remains open through verification. Detected changes invalidate the plan; no replacement plan is silently generated.
- Every successful rename is checked relative to the same held FDs: destination device/inode/type and size/mtime must match, and the source name must be absent in its held parent. Parent directories are synchronized. File contents are not checksummed. Undo uses these identical primitives and refuses changed historical paths rather than searching for relocated files.

### Races prevented by the primitives

**A — Parent pathname substitution:** A substituted directory or symlink before resolution is rejected by safe lookup or parent identity comparison. After opening the parents, renaming a parent pathname or replacing it with a symlink cannot redirect validation, rename, or verification to another directory object. All three use the held directory FDs and single basenames; no parent pathname is reopened between final validation and mutation. The earlier implementation already anchored rename, but re-resolved validation and verification paths; this mismatch is now removed. A relocated parent that remains beneath the root can complete the current operation in the intended directory object. Later operations and undo refuse stale recorded paths.

**B — Destination creation:** Destination absence is checked without following symlinks for diagnostics. If any entry appears afterward, `RENAME_NOREPLACE` refuses atomically. Move and undo never overwrite, including when the occupying entry is a symlink.

### Residual threat assumptions

**C — Source basename replacement:** There is still a short interval between the final metadata check and `renameat2` resolving the source basename. The syscall accepts directory FDs, names, and flags, but no expected inode or expected source-file FD. Keeping the checked source FD open prevents inode recycling, but cannot make rename conditional on that FD. A process with write/search permission to the source parent (subject to sticky-directory rules) could replace that exact directory entry during this interval. A deliberate attack must target this window; an ordinary rename of that exact source could also collide by chance. Unrelated directory activity does not violate this guarantee. If a substitute is renamed, verification detects the different object and stops; it does not roll back or guess. A substituted symlink could itself be renamed in this window, but its target is never followed. Concurrent writes to the exact file are likewise checked through metadata, not locked out.

**Directory topology is distinct from pathname redirection:** A held FD pins a directory object, not its location in the tree. Before the final source check, YakTool walks each held parent's live `..` ancestry through directory FDs to the approved root, refusing unprovable ancestry, mount crossings, or a walk exceeding 1,024 levels. This catches relocation outside the root before that check without reopening a recorded pathname. However, `renameat2` has no atomic root-ancestry constraint: an actor able to rename a held parent or an ancestor out of the root *after* the ancestry check can still cause the operation on that same object to occur outside the root. This requires write/search permission on the directory's containing directories and an outside destination on the same filesystem. Merely substituting a symlink at the old pathname cannot do this. The root-confinement guarantee therefore assumes no such concurrent cross-root topology relocation during the syscall window; it is not an unconditional sandbox guarantee against that actor.

For YakTool's single-user local use, these are targeted same-directory/same-object races, not a claim that all concurrent filesystem activity is unsafe. A malicious process with the same user's directory permissions generally already has authority over these files; YakTool does not isolate against it. No permission changes, security claims based on advisory locks, privileged isolation, or automatic rollback are used to disguise the residuals. Journal integrity also assumes the user's own processes do not tamper with its storage.

The contracts are documented in the Linux [directory FD rationale](https://www.man7.org/linux/man-pages/man2/open.2.html), [openat2 manual](https://www.man7.org/linux/man-pages/man2/openat2.2.html), and [rename manual](https://www.man7.org/linux/man-pages/man2/rename.2.html). The absence of an expected-inode/root-ancestry argument is the reason these separate check-and-rename residuals cannot be eliminated by the selected syscall interface.

## Journal, partial execution, and undo

SQLite lives at `$XDG_DATA_HOME/yaktool/yaktool.db`, falling back to the account home's `.local/share/yaktool/yaktool.db`. Only the application data directories are initialized automatically. The database is a private, singly linked regular file, and its directory must be owned by the user and not writable by other users. Symlink journal paths are rejected. An advisory file lock serializes YakTool writers. Read-only history and plan viewing work while a writer awaits confirmation. Database files and their directory are protected from planned moves.

SQLite uses `synchronous=FULL` and rollback journaling. The exact plan is stored before preview; confirmation is recorded before execution. Each operation has durable attempted state before its syscall, completed state after rename, and verified state after postcondition checks. Errors stop the sequence. Multi-file moves are not atomic transactions and there is no automatic rollback.

History shows the latest 20 transactions, with verified/planned counts. `show-plan` loads stored evidence without resolving the filesystem again and includes recorded outcomes. Results distinguish not attempted, attempted, completed, failed, and verified. A completed rename whose verification failed remains completed, never verified.

Undo selects the newest eligible forward transaction (`verified` or `partial`) and reverses only operations with verified snapshots. It requires the moved file's full post-move snapshot to match, the original name to be absent, unchanged directory identities, and safe same-filesystem paths. The entire inverse plan is checked before starting. No overwrite, auto-renaming, guessing, or substitute file is allowed. Runtime undo failure stops and records partial results; the original is marked `undo_failed` and excluded from automatic selection. A failed/interrupted confirmed undo is not automatically retried.

SQLite and filesystem rename do not share an atomic commit. A crash or persistent storage failure can leave an `attempted` or `completed` record without a verified outcome. The record preserves the exact planned objects and last durable state, but automatic reconciliation/recovery is intentionally absent. Interrupted transactions are not guessed into an undo candidate; inspect them manually. A storage failure before the first durable attempt prevents file mutation. Hardware durability still depends on the filesystem and storage honoring synchronization.

`doctor` does not initialize the journal or rename test files. It reports the account root, UID, data path, read-only SQLite check, and working `openat2` lookup. It explicitly reports no-replace filesystem support as checked on use rather than claiming a successful probe it did not perform.

## Tests and quality checks

```sh
cargo fmt
cargo fmt --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

Every filesystem mutation test owns a temporary directory and injects its approved root; no test moves real Downloads/Documents/Archive files. Sparse files test both sides of the byte limit. Tests inspect contents, paths, device/inode continuity, database records, stale source/parent state, destination races at the kernel boundary, partial execution, partial undo, non-UTF-8 names, quotes, whitespace/control characters, and negative language cases. The permission fixture is skipped when run as root; the cross-device primitive test is skipped only if writable `/dev/shm` on another device is unavailable. Those branches run on the development VPS as an ordinary user.

The existing VPS toolchain lacked rustfmt and Clippy. The implementation session installed Rust 1.85.1 with those components in a temporary, isolated development directory; no system package, shell profile, or runtime dependency was installed. Standard Rust installations should include the rustfmt and Clippy components before running this suite.

## Future model boundary and current limitations

The future path is Natural language → Local model → Canonical Intent → existing deterministic core. A future interpreter's output remains untrusted. Models interpret; tools act; policies decide; verification proves. The model receives no access to filesystem syscalls, SQLite internals, or arbitrary code execution.

Unresolved language may use the local semantic model, but deterministic commands do not contact Ollama. The model cannot access the filesystem, construct plans, choose policy/confirmation, or execute commands. There is no deletion, overwrite, cross-filesystem copy/delete fallback, recursive traversal, hidden-file mutation option, background service, shell, network API, GUI, plugin system, or configuration language. The strict concurrent-writer gap and crash recovery limits above remain open. JSON schemas in `schemas/` document V1; Rust structures and runtime validation are authoritative. No license was selected because the repository did not specify one.
