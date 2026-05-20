#!/bin/bash

USERS=(
    "paliichukvladyslav"
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

    USER_DATA=$(curl -s "https://api.github.com/users/$user")
    AVATAR_URL=$(echo "$USER_DATA" | jq -r '.avatar_url')

    if [ "$AVATAR_URL" != "null" ] && [ -n "$AVATAR_URL" ]; then
        echo "downloading avatar: $user"
        curl -s -o "repos/$user/avatar.png" "$AVATAR_URL"
    fi

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
