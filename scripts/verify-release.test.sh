#!/bin/bash
# Self-test for verify-release.sh against the signed test fixtures, with a stub curl for downloads.
# Exit 0 when every case behaves, 1 otherwise, 3 when it cannot run (no minisign).
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/verify-release.sh"
fixtures="$here/../rust/deskpuck-core/tests/fixtures/release"
command -v minisign >/dev/null || { echo "verify-release.test.sh: CANNOT CHECK: minisign not found" >&2; exit 3; }
work=$(mktemp -d) || exit 3
trap 'rm -rf "$work"' EXIT

failures=0
expect() { # status, description, then the command
    local want=$1 what=$2 got
    shift 2
    "$@" >"$work/out" 2>&1
    got=$?
    if [ "$got" -ne "$want" ]; then
        echo "FAIL: $what: exit $got, want $want" >&2
        sed 's/^/    /' "$work/out" >&2
        failures=$((failures + 1))
    fi
}

# A fresh copy of the signed release: SHA256SUMS, its signature and every file it lists.
release() {
    rm -rf "$work/release" && mkdir "$work/release" &&
        cp "$fixtures/SHA256SUMS" "$fixtures/SHA256SUMS.minisig" "$fixtures"/assets/Deskpuck-0.4.0-* "$work/release/"
}
key="$fixtures/test.pub"

release
expect 0 "a signed release whose files match verifies" "$script" --dir "$work/release" "$key"
grep -q "signature and 4 file(s) verified" "$work/out" || { echo "FAIL: not every file was checked" >&2; failures=$((failures + 1)); }
expect 1 "the release key refuses the test key's signature" "$script" --dir "$work/release"

for signature in other-key.minisig forged-id.minisig legacy.minisig; do
    release && cp "$fixtures/$signature" "$work/release/SHA256SUMS.minisig"
    expect 1 "$signature is refused" "$script" --dir "$work/release" "$key"
done

release && printf x >>"$work/release/Deskpuck-0.4.0-setup.exe"
expect 1 "a changed file is refused" "$script" --dir "$work/release" "$key"
release && rm "$work/release/Deskpuck-0.4.0-macos-arm64.zip"
expect 1 "a listed file that is missing is refused" "$script" --dir "$work/release" "$key"
release && rm "$work/release/SHA256SUMS.minisig"
expect 1 "an unsigned release is refused" "$script" --dir "$work/release" "$key"
release && printf '\n' >>"$work/release/SHA256SUMS"
expect 1 "a SHA256SUMS changed after signing is refused" "$script" --dir "$work/release" "$key"

expect 2 "a tag that is not vN.N.N is bad usage" "$script" "v1.0.0/../x"
expect 3 "a missing key cannot check" "$script" --dir "$work/release" "$work/no-such.pub"

# Downloads: the stub serves the fixtures, fails with STUB_EXIT, and logs each URL.
mkdir -p "$work/bin"
cat >"$work/bin/curl" <<'STUB'
#!/bin/bash
for arg; do last=$arg; done
while [ $# -gt 0 ]; do [ "$1" = -o ] && out=$2; shift; done
echo "$last" >>"$STUB_LOG"
[ "${STUB_EXIT:-0}" -eq 0 ] || exit "$STUB_EXIT"
cp "$STUB_SERVE/${last##*/}" "$out" 2>/dev/null || exit 22
STUB
chmod +x "$work/bin/curl"
release
download() { PATH="$work/bin:$PATH" STUB_SERVE="$work/release" STUB_LOG="$work/log" "$@" "$script" v0.4.0 "$key"; }

: >"$work/log"
expect 0 "a published release downloads and verifies" download env
want="https://github.com/Pyronewbic/deskpuck/releases/download/v0.4.0/SHA256SUMS.minisig"
grep -qx "$want" "$work/log" || { echo "FAIL: did not fetch $want" >&2; failures=$((failures + 1)); }
[ "$(grep -c . "$work/log")" -eq 6 ] || { echo "FAIL: want 6 downloads, got $(grep -c . "$work/log")" >&2; failures=$((failures + 1)); }
expect 3 "a network failure cannot check, never passes" download env STUB_EXIT=6
expect 1 "a release without the file fails" download env STUB_EXIT=22
rm "$work/release/SHA256SUMS.minisig"
expect 1 "a published release without a signature fails" download env

if [ $failures -ne 0 ]; then
    exit 1
fi
echo "verify-release.sh: all cases behave"
