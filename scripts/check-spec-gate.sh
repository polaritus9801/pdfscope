#!/usr/bin/env bash
# CI half of the spec gate: a pull request that changes product code must change a spec.
#
# The commit-msg hook covers the local case; this covers the case where someone used --no-verify,
# or pushed commits made on another machine. Failing here is intentional: it is cheaper to fix a
# missing document than an unreviewable design.
set -euo pipefail

base="${1:?base commit required}"
head="${2:?head commit required}"

spec_dir="docs/specs"
code_pattern='^(crates/[^/]+/src/|ui/src/)'

changed=$(git diff --name-only "$base" "$head")

changed_code=$(printf '%s\n' "$changed" | grep -E "$code_pattern" || true)
if [ -z "$changed_code" ]; then
    echo "no product code changed; spec gate not applicable."
    exit 0
fi

changed_specs=$(printf '%s\n' "$changed" \
    | grep -E "^$spec_dir/[0-9]{4}-.*\.md$" \
    | grep -v 'template\.md$' \
    || true)

if [ -n "$changed_specs" ]; then
    echo "spec gate satisfied by:"
    printf '  %s\n' $changed_specs
    exit 0
fi

{
    echo "spec gate failed: product code changed but no spec was touched"
    echo
    printf '  code:  %s\n' $changed_code
    echo
    echo "  Add or update a file under docs/specs/ in this pull request."
    echo "  See AGENTS.md §2."
} >&2

exit 1
