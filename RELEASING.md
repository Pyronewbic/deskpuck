# Releasing

Releases are built on the maintainer's Mac, which holds the Mac signing certificate and the minisign release key (`~/.minisign/deskpuck.key`; `brew install minisign`). `scripts/make-dmg.sh` also needs Python 3 (it installs a pinned `dmgbuild` into `.venv`).

1. Move the Unreleased notes in [CHANGELOG.md](CHANGELOG.md) under the new version, and bump `VERSION` in `scripts/make-app.sh` with `version` in `rust/Cargo.toml` (a test keeps them equal), then refresh `rust/Cargo.lock` with `cargo update --workspace`. Merge to main through a pull request.
2. `gh workflow run release-artifacts.yml --ref main` builds and attests the Linux and Windows downloads.
3. Once it passes, `scripts/release.sh --publish` with `GH_TOKEN` set runs every check, verifies those downloads' checksums and attestations, builds and signs the Mac app, signs `SHA256SUMS` with the minisign key (it asks for the passphrase), pushes a signed tag and creates a draft release with the changelog notes. Review the draft, then publish it on GitHub.
4. `scripts/aur-update.sh` points `packaging/aur/deskpuck-bin` at the published release, after checking the Linux tarball's checksum and attestation; it needs `makepkg` or Docker to write `.SRCINFO`. Merge that change through a pull request, then copy the folder's files to the `deskpuck-bin` repository on the AUR and push there.

## The release key

`packaging/minisign.pub` is built into every app, which installs no update without its signature. If the secret key is lost or compromised, make a new pair with `minisign -G -f -p packaging/minisign.pub -s ~/.minisign/deskpuck.key` and release as usual; installed copies then refuse the update, and their users update by hand once. After a compromise, also publish a security advisory and note the rotation in [SECURITY.md](SECURITY.md).
