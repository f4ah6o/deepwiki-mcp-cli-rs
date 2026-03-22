# Release new version (CalVer: YYYY.M.PATCH, no v-prefix)

release-check:
    cargo test --all --all-features
    cargo build --release
    cargo publish --dry-run

release: release-check
    version=$(grep -m1 '^version = ' Cargo.toml | cut -d '"' -f 2); \
    git tag "${version}"; \
    git push origin "${version}"
