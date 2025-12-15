#!/bin/bash
# Test AUR API directly with curl

echo "==> Testing AUR RPC API..."
echo

echo "1. Searching for 'yay'..."
curl -s "https://aur.archlinux.org/rpc/v5?v=5&type=search&arg=yay" | jq -r '.results[:5] | .[] | "\(.Name) \(.Version) - \(.Description)"'
echo

echo "2. Getting info for 'paru'..."
curl -s "https://aur.archlinux.org/rpc/v5?v=5&type=info&arg=paru" | jq '.results[0] | {Name, Version, Description, Maintainer, NumVotes, Depends, MakeDepends}'
echo

echo "3. Getting git clone URL for paru..."
echo "https://aur.archlinux.org/paru.git"
echo

echo "==> AUR API test complete!"
