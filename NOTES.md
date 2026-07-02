# Local notes — Codex CLI source build (not upstream docs)

Knowledge accumulated while standing this up against the ycluster inference
gateway (2026-07-02). Carried on our local branch (do not push upstream).

## What this clone is

- Upstream `openai/codex`, built from source. ~95% Rust (`codex-rs/` workspace);
  the `codex` binary comes from the `cli` crate (`codex-cli`).
- **Local divergence:** commit `c574659d58` "core: fall back to flattened name
  when resolving MCP tool calls" (see below). Rebase/reapply after pulls; a
  diff copy lives at `~/proj/ycluster/tmp/codex-flat-toolname-fallback.patch`.
- Uncommitted WIP: `codex-rs/model-provider-info/src/lib.rs` (disabling the
  built-in openai provider — session exploration, not settled).

## Building

```bash
cd codex-rs && cargo build --release --bin codex
```

- **GOTCHA (cost an afternoon): the `~/bin/codex` wrapper must point at the SAME
  profile you rebuild.** It once pointed at `target/debug/codex` while all
  rebuilds targeted `target/release/codex` — so every change silently ran stale
  and appeared to have no effect. It now points at `target/release/codex`.
  Debug is also slow at runtime for an agent. When a code change seems to have
  no effect, FIRST confirm which binary actually runs: `cat "$(command -v
  codex)"` and compare its target's mtime to your build. A running interactive
  session keeps the binary it started with — restart it after a rebuild.

- The low-memory profile lives in `.cargo/config.toml` at the repo root
  (carried commit), so plain `cargo build --release` is safe. Background:
  upstream's `[profile.release] lto = "thin"` pulls the whole dep graph into
  one rustc during the final link and gets OOM-killed on this box (31 GiB RAM,
  swap near-full); the override sets `lto = false, codegen-units = 256`.
  Full build ~15 min; incremental (core change) ~4 min.
- Toolchain pinned by `rust-toolchain.toml` (rustup auto-installs).
- Binary: `codex-rs/target/release/codex` (~1.6 GB with line-tables debuginfo).

## Runtime wiring

- `~/bin/codex` — wrapper: injects `DEV1_TOKEN` from `~/proj/ycluster/dev1.token`,
  execs the release binary.
- `~/.codex/config.toml` — provider `dev1` (`https://dev1.ycluster.net/v1`,
  `wire_api = "responses"`), model `minimax-m3` (vLLM 0.24, MXFP4 + EAGLE3),
  `model_context_window = 256000` (matches server `--max-model-len`).
- The "model metadata not found / fallback metadata" warning is patched out
  (carried commit in `turn_context.rs`) when `model_context_window` is set
  explicitly — custom-provider slugs are never in the catalog, so it nagged
  every turn. The heavyweight alternative (`model_catalog_json`) requires
  owning `base_instructions` (the system prompt) per entry; not worth it.
- Current Codex config scheme: the base `config.toml` IS the default profile;
  top-level `profile = "..."` is rejected as legacy. Named profiles live in
  `<name>.config.toml` selected with `--profile`.

## The carried patch: MCP tool dispatch vs OpenAI-compatible providers

`core/src/tools/registry.rs::ToolRegistry::tool()` — commit `c574659d58`.

- Codex registers MCP tools under a structured key
  `ToolName { name: "kagi_search_fetch", namespace: Some("mcp__kagi") }`
  and advertises them via the Responses "namespace" extension (a group named
  `mcp__kagi` with bare member names).
- OpenAI models call back with separate `name` + `namespace` wire fields.
  **Stock vLLM does not emit the `namespace` field** — the model calls with a
  single flat string `mcp__kagi__kagi_search_fetch`, `namespace: None`, which
  hash-misses the structured HashMap key → `"unsupported call: …"` for every
  MCP tool (and hosted `web_search`). Built-in tools are registered plain, so
  shell/file tools work — the failure is MCP-specific.
- Subtlety that broke fix #1: registered namespaces have **no trailing
  delimiter**, and both `ToolName::Display` and `flat_tool_name()` concatenate
  namespace+name **without inserting `__`** (`"mcp__kagi" + "kagi_search_fetch"
  = "mcp__kagikagi_search_fetch"` ≠ the called name). The fallback must
  re-derive the joined wire form: `ns.trim_end_matches('_') + "__" +
  name.trim_start_matches('_')` (mirrors private `join_tool_name` in
  `core/src/tools/handlers/mcp.rs`). The patch tries that join and raw concat,
  only on the miss path with `namespace: None`.
- Would be a legitimate upstream issue: "MCP tools unusable with OpenAI-
  compatible providers that don't echo the namespace field."

## vLLM 0.24 Responses-API quirks (server-side, not Codex bugs)

All specific to `/v1/responses`; the Chat Completions path is fine (proven:
opencode + same backend works, including array tool args).

1. **Array-valued tool args get mangled**: `queries: list[str]` arrives as
   `{"queries": {"item": [...]}}` → pydantic validation always fails. Give
   tools scalar-only args when the caller is Codex.
2. **Dropped tool calls**: some turns end right after reasoning without
   emitting the call (upstream vllm#27263). Prompt-shape sensitive: explicit
   "Call <tool> … you MUST" lands far more reliably than "use the X tool".
   Only real fix: Responses→Chat translation shim in front of the gateway, or
   newer vLLM.
3. Hosted `web_search` tool: not implemented by vLLM → keep
   `web_search = "disabled"` in config; use an MCP search tool instead.
4. vLLM server needs `--enable-auto-tool-choice --tool-call-parser minimax_m3`
   (it has them; see ycluster `minimax-m3-stock-start.sh.j2`).

## Kagi MCP

- `~/bin/kagi-mcp-v0.py` — local shim, scalar args (`query: str`), Kagi
  **v0** Search API with **`Authorization: Bot`**. Reasons:
  - kagimcp (PyPI) >= 1.0 targets `/api/v1` + Bearer → **401 for our key**
    (same root cause as the Open-WebUI `kagi.py` override carried in ycluster;
    empirically only v0+Bot works — v1 returns `general.unauthorized` with
    both schemes).
  - kagimcp 0.1.5 (last v0 release) declares `queries: list[str]` → unusable
    via the Responses array-mangling above. (opencode pins 0.1.5 successfully
    because it rides Chat Completions.)
- Token: `/data/home/dev/kagi.token` (from kagi.com/api/keys).

## Sandbox / approvals on Ubuntu 24.04

- `kernel.apparmor_restrict_unprivileged_userns=1` doesn't block userns
  creation outright — it strips capabilities in the child, so bwrap fails
  *late* with `bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted`.
  Net effect: every sandboxed command fails → Codex prompts for EVERYTHING.
- Fix: AppArmor grant `~/admin/codex-userns` → `/etc/apparmor.d/codex-userns`,
  covering the main binary AND the re-exec'd helpers
  `~/.codex/tmp/arg0/*/codex-linux-sandbox` + `codex-execve-wrapper`
  (the helpers create the namespaces; profiles on the main binary alone are
  insufficient). `flags=(unconfined) { userns, }` = Ubuntu's intended pattern.
- Gotchas:
  - Profiles attach at exec: restart running codex processes after
    `apparmor_parser -r`.
  - Threads from the broken era are poisoned: the model learned to send
    `sandbox_permissions: "require_escalated"` on every exec_command (which
    forces a prompt even with a working sandbox). Start fresh threads instead
    of `codex resume` across the fix, or tell the model to stop escalating.
  - Rebuild to a different binary path ⇒ update the profile.
- Headless `codex exec` cannot answer approval prompts: MCP calls get
  auto-cancelled unless auto-approved. Options: `-s danger-full-access`
  (one-offs), or per-tool `[mcp_servers.<name>.tools.<tool>] approval_mode =
  "approve"` in config.
- The sandbox is worth the setup: restricted network correctly blocked
  justfile auto-tool-downloads (`just fmt`).

## Useful diagnostics

- Session transcripts: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` —
  `function_call` / `function_call_output` records show exact tool names,
  raw arguments (escalation flags!), and outputs.
- Runtime logs: `~/.codex/logs_2.sqlite`, table `logs`, text in
  `feedback_log_body` (SSE traces show the request `tools` array — the ground
  truth for what names/schemas the model was shown).
- `codex doctor` — config parse status, resolved model/provider, sandbox
  status, helper paths.
- AppArmor label of a running process: `cat /proc/<pid>/attr/current`.
