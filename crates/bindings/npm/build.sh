#!/bin/sh
# Build the npm package: one wasm module per target, and a package.json that
# points at both.
#
# Two builds because the loaders differ. The node build reads the .wasm off
# disk at import; the web build takes it as a URL a bundler rewrites. One
# build serving both means a conditional import that every bundler gets wrong
# in a different way.
set -eu

cd "$(dirname "$0")"
crate=..
out="$(pwd)/dist"

command -v wasm-pack >/dev/null 2>&1 || {
	echo "npm/build.sh: wasm-pack is not on PATH" >&2
	exit 69
}

version=$(grep -m1 '^version' "$crate/../../Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')

rm -rf "$out"
mkdir -p "$out"

for target in nodejs web; do
	case "$target" in
	nodejs) dir=node ;;
	*) dir=web ;;
	esac
	wasm-pack build "$crate" \
		--target "$target" \
		--out-dir "$out/$dir" \
		--out-name catena \
		--release \
		-- --features wasm --no-default-features
	# wasm-pack writes one of these per target and they disagree; the
	# package.json below is the one that ships.
	rm -f "$out/$dir/package.json" "$out/$dir/README.md" "$out/$dir/.gitignore"
done

# The node build is CommonJS and the package declares itself ESM, so the entry
# has to say so in its extension or node reads `exports.x = …` as a syntax
# error. The web build is already ESM and keeps its .js.
mv "$out/node/catena.js" "$out/node/catena.cjs"
mv "$out/node/catena.d.ts" "$out/node/catena.d.cts"

sed "s/@VERSION@/$version/" package.json.in >"$out/package.json"
cp README.md "$out/README.md"
cp ../LICENSE "$out/LICENSE"

echo "built $out (version $version)"
echo "publish with: npm publish $out --access public"
