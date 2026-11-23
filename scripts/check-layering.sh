#!/usr/bin/env bash
set -euo pipefail

# AC-09: non-UI crates must not depend on GPUI.
crates=(md-core md-document md-markdown md-editor md-layout)

failed=0
for crate in "${crates[@]}"; do
    tree="$(cargo tree -p "$crate" --edges normal --prefix none)"
    if printf '%s\n' "$tree" | grep -Eq '^gpui( |$)'; then
        echo "layering violation: $crate depends on gpui" >&2
        failed=1
    fi
done

if ((failed)); then
    exit 1
fi

echo "layering: OK (no GPUI below md-render)"
