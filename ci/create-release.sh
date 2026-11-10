#!/usr/bin/env bash
set -x
name="$1"

notes=$(cat <<EOT
See https://wezterm.org/changelog.html#$name for the changelog

If you're looking for nightly downloads or more detailed installation instructions:

[macOS](https://wezterm.org/install/macos.html)
EOT
)

gh release view "$name" || gh release create --prerelease --notes "$notes" --title "$name" "$name"
