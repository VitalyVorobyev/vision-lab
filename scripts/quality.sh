#!/usr/bin/env bash
set -euo pipefail

if command -v rg >/dev/null 2>&1; then
  allow_matches=$(rg -n --glob '*.rs' '#!?\[allow\(' . || true)
else
  allow_matches=$(find . -name '*.rs' -not -path './target/*' -print0 \
    | xargs -0 grep -nE '#!?\[allow\(' || true)
fi

if [[ -n "$allow_matches" ]]; then
  echo "$allow_matches"
  echo "Rust allow attributes are forbidden; remove the attribute or refactor the code." >&2
  exit 1
fi

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
audit_ignores=(
  RUSTSEC-2024-0370
  RUSTSEC-2024-0411
  RUSTSEC-2024-0412
  RUSTSEC-2024-0413
  RUSTSEC-2024-0414
  RUSTSEC-2024-0415
  RUSTSEC-2024-0416
  RUSTSEC-2024-0417
  RUSTSEC-2024-0418
  RUSTSEC-2024-0419
  RUSTSEC-2024-0420
  RUSTSEC-2024-0429
  RUSTSEC-2024-0436
  RUSTSEC-2025-0075
  RUSTSEC-2025-0080
  RUSTSEC-2025-0081
  RUSTSEC-2025-0098
  RUSTSEC-2025-0100
)
audit_args=(--target-os macos)
for advisory in "${audit_ignores[@]}"; do
  audit_args+=(--ignore "$advisory")
done
cargo audit "${audit_args[@]}"
cargo deny check

bun install --cwd apps/operator-ui/ui --frozen-lockfile
bun run --cwd apps/operator-ui/ui lint
bun run --cwd apps/operator-ui/ui typecheck
bun run --cwd apps/operator-ui/ui build
(cd apps/operator-ui/ui && bun audit)
