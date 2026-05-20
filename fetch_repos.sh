#!/bin/bash

USERS=(
    "paliichukvladyslav"
    "64-bite"
    "Mortyfera"
    "MCPHackers"
)

if ! command -v jq &> /dev/null; then
    echo "error: jq is required"
    exit 1
fi

mkdir -p repos

for user in "${USERS[@]}"; do
    echo "processing profile: $user"
    mkdir -p "repos/$user"

    REPOS_URLS=$(curl -s "https://api.github.com/users/$user/repos?per_page=100" | jq -r '.[] | .clone_url')

    if [ -z "$REPOS_URLS" ] || [ "$REPOS_URLS" == "null" ]; then
        echo "warning: no repos found or api limit reached for $user"
        continue
    fi

    for url in $REPOS_URLS; do
        repo=$(basename "$url" .git)

        if [ -d "repos/$user/$repo" ]; then
            echo "exists: $repo"
        else
            echo "cloning: $repo"
            git clone "$url" "repos/$user/$repo"
        fi
    done
done

echo "done"
