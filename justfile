# Release new version (CalVer: YYYY.M.PATCH, no v-prefix)

release-check:
    cargo test --all --all-features
    cargo build --release
    cargo publish --dry-run

release: release-check
    git tag -f latest HEAD
    git push -f origin latest
