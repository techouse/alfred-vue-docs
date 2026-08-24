#!/usr/bin/env bash

set -euo pipefail

missing_names=()
for variable_name in ALGOLIA_APPLICATION_ID ALGOLIA_SEARCH_ONLY_API_KEY ALGOLIA_SEARCH_INDEX; do
  if [[ -z "${!variable_name+x}" ]]; then
    missing_names+=("$variable_name")
  fi
done

if (( ${#missing_names[@]} > 0 )) && [[ -f ./.env ]]; then
  dotenv_exports="$(
    set +e
    set -a
    source ./.env
    dotenv_status=$?
    set +a
    set -e
    if (( dotenv_status != 0 )); then
      exit "$dotenv_status"
    fi
    for variable_name in "${missing_names[@]}"; do
      if [[ -n "${!variable_name+x}" ]]; then
        printf '%s=%q\n' "$variable_name" "${!variable_name}"
      fi
    done
  )"
  while IFS= read -r assignment; do
    if [[ -n "$assignment" ]]; then
      eval "export $assignment"
    fi
  done <<< "$dotenv_exports"
fi

for variable_name in ALGOLIA_APPLICATION_ID ALGOLIA_SEARCH_ONLY_API_KEY ALGOLIA_SEARCH_INDEX; do
  if [[ -z "${!variable_name:-}" ]]; then
    echo "$variable_name must be set in the environment or .env file" >&2
    exit 1
  fi
done

for variable_name in ALGOLIA_APPLICATION_ID ALGOLIA_SEARCH_ONLY_API_KEY ALGOLIA_SEARCH_INDEX; do
  export "$variable_name"
done

cargo build --release --locked
