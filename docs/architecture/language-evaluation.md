# DStack implementation language evaluation

Rust has been approved for DStack v2, beginning at package version `2.0.0`.
This document retains the comparison that led to that decision and the
constraints that keep DStack substantially smaller than IRIS.

## What DStack needs from a language

The future core is a small-to-medium cross-platform CLI. Its hard problems are
reliable process argv, native paths, structured and locked state, terminal
interaction, deterministic parsing, release artifacts, and maintainable shell
integration. It does not need a garbage-free hot loop, kernel integration, a
GUI, or an embedded runtime.

The most important evaluation criteria are:

- first-class Linux, macOS, and native Windows process/path APIs;
- one binary with practical x86_64 and ARM64 release builds;
- safe exact-argv child execution and stream attachment;
- reliable serialization, locking, atomic replacement, Unicode, signals, and
  TTY detection;
- readable command/domain types and tests;
- a sustainable release process for the current maintainer;
- low enough implementation complexity for DStack's actual scope.

Shell adapters are required in every option. The implementation language
cannot directly mutate a parent shell.

## Summary comparison

| Area | Go | Rust | C |
|---|---|---|---|
| Cross-platform standard library | Excellent process, path, JSON, HTTP, and OS support | Strong process/path/I/O; mature crates fill CLI, dirs, serialization, locking | APIs differ heavily by platform; substantial wrappers required |
| Single binary | Straightforward; commonly static except platform/cgo choices | Straightforward; libc/static-link choices need deliberate targets | Small native binary, but library/runtime packaging is manual |
| Cross-compilation | Very simple for pure Go | Good, but target linkers and libc choices add work; native runners avoid much of it | Toolchain-heavy, especially Windows/macOS cross builds |
| CLI parsing | Cobra/urfave or standard flag packages; subcommands need a library or code | Clap derive gives typed subcommands, help, completions | Must select/vendor a parser or write one |
| Process argv/streams | `os/exec` is clear; Windows quoting handled by runtime | `std::process::Command` is exact and explicit | `fork/exec` versus `CreateProcessW`; Windows command-line encoding is easy to get wrong |
| Paths and Unicode | `filepath`; Windows strings are UTF-8 abstractions with OS conversion | `PathBuf`/`OsString` preserve non-UTF-8 Unix paths and Windows wide paths | Manual encoding and ownership; high defect risk |
| Memory safety | Garbage-collected memory safety | Compile-time memory safety | Manual memory/resource safety |
| Data modeling | Simple structs/interfaces; good readability | Enums/results make resolution and error states precise | Manual tagged unions/error conventions |
| Serialization/config | Excellent standard JSON; TOML library needed | Serde ecosystem is strong; TOML straightforward | External parser and careful ownership required |
| Locking/atomic writes | Good OS APIs/libraries; Windows cases still need care | Good crates and explicit error handling; Windows cases still need care | Must implement platform branches directly |
| TTY/signals | Solid libraries; Windows console behavior still specialized | Mature crates; direct control and typed error propagation | Full control, but greatest platform code burden |
| Startup latency | Fast | Fast | Fastest, but difference is immaterial beside Docker startup |
| Binary size | Usually larger due to runtime | Moderate to large depending on dependencies/LTO | Potentially smallest |
| Build speed | Fast | Slower clean builds | Fast source compile, but build-system/toolchain complexity is higher |
| Dependency management | Modules are simple and reproducible | Cargo lockfiles/tooling are excellent | No universal package manager; vendoring/build integration required |
| Testing | Excellent built-in test tooling | Excellent unit/integration/property tooling | Framework/tooling choice required |
| Contributor accessibility | Generally easiest of the three | Steeper ownership/type learning curve | Familiar syntax but unsafe/platform-heavy implementation |
| Maintainer evidence | No project-local example | IRIS demonstrates current Rust, Cargo, CI, and release capability | No repository evidence and highest bespoke burden |

## Go

### Strengths for DStack

- Pure-Go release builds for common OS/architecture pairs are exceptionally
  simple and fast.
- `os/exec`, `filepath`, `os.UserConfigDir`, JSON, HTTP, and testing cover much
  of DStack with little third-party code.
- The language is easy to review and is well suited to a CLI dominated by I/O.
- Goroutines are useful for process signal handling or bounded concurrent
  discovery, although DStack does not require much concurrency.
- Garbage collection overhead is negligible for a short-lived Docker wrapper.

### Costs and risks

- A quality subcommand hierarchy, shell completions, TOML, file locking, and
  terminal behavior still introduce external packages or custom code.
- Go's string/path model is convenient but does not preserve arbitrary
  non-UTF-8 Unix path bytes as naturally as Rust's `OsString`. DStack could
  explicitly support Unicode/valid platform strings and reject edge cases, but
  that is a product choice.
- The maintainer would operate a second compiled ecosystem rather than reusing
  established Cargo/release habits from IRIS.
- Interface-heavy architecture is easy to overuse in Go. DStack needs only a
  few I/O boundaries, not a framework of mocks.

### Expected complexity

Lowest initial implementation complexity. A single module with packages for
CLI, resolution, registry, Docker, platform, and shell generation would be
sufficient. Go is the strongest alternative if the maintainer values build
simplicity and contributor approachability over cross-project tooling reuse.

## Rust

### Strengths for DStack

- `PathBuf`, `OsString`, `Command`, enums, and `Result` model DStack's main
  correctness boundaries directly: target source, file selection, cancellation,
  process failure, and intentional divergence.
- Clap can define the proposed hierarchy, generate help/completions, and avoid
  hand-written positional parsing.
- Serde plus TOML supports an escaped, versioned registry/config format.
- The type system makes it difficult to pass a resolver diagnostic where a
  Compose path is required—the exact class of current mapfile defect.
- Cargo provides locked dependencies, unit/integration tests, formatting,
  linting, and native-target release workflows already familiar from IRIS.
- No runtime/GC is needed and startup is fast.

### Costs and risks

- Rust has the steepest learning curve of the memory-safe options and slower
  clean builds.
- Careless use of traits, generics, async, workspaces, or many crates would be
  disproportionate to DStack.
- Cross-compiling GNU Linux or macOS from one host can be cumbersome. Native CI
  runners, as IRIS uses, avoid most of that complexity.
- Windows console, replacement, and locking still require targeted testing;
  Rust does not make platform semantics disappear.
- A self-updater or network client would add dependencies. The first release
  should leave update/rollback to repository-owned installers.

### Expected complexity

Moderate. Use one package and one binary, not a workspace. Keep the module
graph shallow and use concrete types. Traits are justified at the process,
filesystem/lock, and terminal test boundaries only if simple injected
functions are insufficient. Avoid async; DStack's operations are sequential.

## C

### Strengths for DStack

- Small binaries, minimal runtime, exact OS control, and broad compiler
  availability.
- Direct access to POSIX and Win32 process/console/filesystem APIs.
- Startup and resource costs are minimal.

### Costs and risks

- DStack would need separate `fork`/`exec` and `CreateProcessW` code, correct
  Windows quoting, UTF-8/UTF-16 conversion, path handling, signal/console
  handling, serialization, file locking, and safe dynamic collections.
- CLI parsing, TOML, checksums/networking, test harnesses, dependency vendoring,
  and cross-platform builds all require more decisions and maintenance.
- Manual memory and resource management add risk without serving a performance
  need.
- A C implementation would likely become more code than the product logic and
  would be harder for occasional contributors to modify safely.

### Expected complexity

Highest by a large margin. C is justified only if tiny binaries, embedded
constraints, or direct low-level integration dominate; none currently does.
It is not recommended.

## Platform and release implications

### Linux and macOS

Go and Rust both provide native filesystem/process APIs. Rust release targets
can mirror IRIS's native runner matrix. Linux linkage must be explicit: a GNU
build is appropriate for mainstream distributions; a musl artifact can be
added only after Docker execution and DNS/update needs are tested. Go can
usually emit a portable pure-Go binary with less linkage discussion.

### Windows

Both Go and Rust can produce native x86_64 and ARM64 executables. Tests must
cover `docker.exe`, Unicode/UNC paths, AppData, atomic replacement, file locks,
console cancellation, and PowerShell initialization. Neither language should
route native support through MSYS. Rust's native `Command` and path types are
adequate; Go's equivalents are equally practical.

### Distribution

Both support zipped/tarred binaries and checksums. Go has easier cross-builds;
Rust has a proven project-local native runner workflow. C would require a build
system, compiler/runtime decisions, and packaging from scratch.

## Shell integration

Language choice does not remove the parent-shell boundary. Each candidate can
print generated initialization and write session state. Shell adapters should
be static templates embedded in the binary, parameterized only by versioned
protocol values. Do not invoke the shell to run Docker. Hostile path/name tests
must cover every adapter.

Rust's enums can define the shell kind and session protocol; Go can do so with
constants and structs. C would need bespoke escaping and ownership code, which
is a material security disadvantage.

## Lessons from IRIS

IRIS is concrete evidence that the maintainer already uses:

- a single Cargo package with a thin `main` and library modules;
- Clap-derived help/version and typed `PathBuf` operands;
- Serde/TOML configuration with a legacy config fallback;
- `cargo fmt`, Clippy with warnings denied, locked tests, and native OS builds;
- tag-to-manifest version verification, checksummed archives, native Windows
  packaging, and draft GitHub Releases.

Those patterns lower the operational cost of Rust for DStack. They do not make
IRIS's dependencies or module count appropriate. DStack should not copy
Ratatui, image/PDF/rendering modules, automatic theme installation, Linux
`/proc` behavior, nFPM packages at launch, or a Rust version merely because
IRIS currently needs it.

IRIS also shows gaps DStack must solve separately: its prebuilt update flow is
manual replacement, it has no shell adapter or rollback, its Makefile install
is Unix-only, and its release matrix currently lacks Windows ARM64. DStack's
installer architecture cannot simply be copied from its README.

## Recommendation

Rust was selected, narrowly, for these DStack-specific reasons:

1. Typed target/selection/error states directly prevent several characterized
   defect classes.
2. Native path and argv handling fit Linux, macOS, and Windows without a shell.
3. The maintainer has current Cargo, Clap, config, test, and release experience
   in IRIS, reducing operational risk.
4. The release model can reuse proven concepts while remaining a separate,
   much smaller package.

Go is a credible alternative and should be selected instead if the maintainer
prioritizes simplest cross-compilation, faster builds, and a gentler contributor
curve. The decision should not be reopened after migration code begins without
an explicit cost review.

The approved Rust implementation carries these constraints:

- one package, one binary, no workspace initially;
- no async runtime;
- minimal dependencies, each justified by portability or correctness;
- no generic plugin system;
- native CI runners for release targets;
- core modules sized to DStack, not modeled after IRIS's TUI architecture.

## Approved language

**Decision: Rust. Runner-up considered: Go. C is not selected.**

Primary tradeoff: Rust offers stronger domain/path correctness and existing
maintainer infrastructure at the cost of steeper implementation and build
complexity. Go offers the simplest delivery path with somewhat less expressive
modeling and no existing project-local tooling reuse.
