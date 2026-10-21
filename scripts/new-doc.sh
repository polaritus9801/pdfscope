#!/usr/bin/env bash
# Scaffold a numbered document from its template.
#
#   ./scripts/new-doc.sh spec core-anchors
#   ./scripts/new-doc.sh adr why-sqlite
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <spec|adr> <slug>" >&2
    exit 2
fi

kind="$1"
slug="$2"

case "$kind" in
    spec) dir="docs/specs" ;;
    adr)  dir="docs/adr" ;;
    *)    echo "unknown kind: $kind (expected spec or adr)" >&2; exit 2 ;;
esac

if ! printf '%s' "$slug" | grep -Eq '^[a-z0-9]+(-[a-z0-9]+)*$'; then
    echo "slug must be lower-case words separated by single dashes: got '$slug'" >&2
    exit 2
fi

mkdir -p "$dir"

last=$(find "$dir" -maxdepth 1 -name '[0-9][0-9][0-9][0-9]-*.md' -print 2>/dev/null \
    | sed -E 's#.*/([0-9]{4})-.*#\1#' | sort -n | tail -1)
next=$(printf '%04d' $((10#${last:-0} + 1)))

target="$dir/$next-$slug.md"
if [ -e "$target" ]; then
    echo "already exists: $target" >&2
    exit 1
fi

sed -e "s/^# NNNN — <title>/# $next — <title>/" \
    -e "s/YYYY-MM-DD/$(date +%Y-%m-%d)/" \
    "$dir/0000-template.md" > "$target"

printf 'created %s\n' "$target"
printf 'remember: status stays `draft` until a human approves it.\n'
