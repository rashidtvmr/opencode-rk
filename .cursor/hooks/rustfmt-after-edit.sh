#!/bin/bash
# Post-edit hook: run rustfmt on edited .rs files
input=$(cat)
file=$(echo "$input" | jq -r '.path // empty')

# Only run on Rust files
if [[ "$file" == *.rs ]]; then
    rustfmt "$file" 2>/dev/null || true
fi

exit 0
