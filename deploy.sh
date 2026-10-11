#!/usr/bin/env bash
# Publish the 3D game to https://moaswift.no/invaders-3d/ (S3 + CloudFront).
#
#   ./deploy.sh --dry-run   show what would change
#   ./deploy.sh             upload, then clear the cache for the game's path
#
# Build first (see README): both flavours go to dist-release/{gpu,gl}.
# Needs the AWS CLI, brotli (brew install brotli) and a signed-in session: AWS_PROFILE=moaswift (aws login --profile moaswift).
set -euo pipefail

BUCKET="moaswift"
REGION="eu-west-1"
DISTRIBUTION="E1MUDCSWW05U01"
PREFIX="invaders-3d"
ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT="$ROOT/dist-release"
STAGE="$ROOT/dist-stage"

DRY=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done

[ -d "$OUT/gpu" ] && [ -d "$OUT/gl" ] || { echo "Missing $OUT/{gpu,gl}: build both flavours first." >&2; exit 1; }
command -v brotli >/dev/null || { echo "Missing brotli: brew install brotli" >&2; exit 1; }
aws sts get-caller-identity >/dev/null 2>&1 || { echo "Not signed in to AWS. Run: aws login --profile moaswift" >&2; exit 1; }

# Stage a copy: the loader on top, the two builds below it, wasm Brotli-compressed once (about 35% smaller than gzip).
rm -rf "$STAGE" && mkdir -p "$STAGE"
cp "$ROOT/loader/index.html" "$STAGE/index.html"
for f in gpu gl; do
  cp -R "$OUT/$f" "$STAGE/$f"
  for w in "$STAGE/$f"/*.wasm; do brotli -q 11 -f -o "$w.br" "$w" && mv "$w.br" "$w"; done
done

DEST="s3://$BUCKET/$PREFIX"
sync() { aws s3 sync "$STAGE" "$DEST" --region "$REGION" --delete "$@" ${DRYFLAG:+--dryrun}; }
[ "$DRY" -eq 1 ] && DRYFLAG=1 || DRYFLAG=""

# 1. wasm: hashed filename, so cache forever; stored Brotli-compressed with the right type.
sync --exclude "*" --include "*.wasm" --content-type "application/wasm" --content-encoding br \
     --cache-control "public, max-age=31536000, immutable"
# 2. hashed scripts: cache forever.
sync --exclude "*" --include "*.js" --cache-control "public, max-age=31536000, immutable"
# 3. everything else (pages, manifest, icon): short cache.
sync --exclude "*.wasm" --exclude "*.js" --cache-control "public, max-age=300"

[ "$DRY" -eq 1 ] && { echo "== Dry run only. Nothing changed."; exit 0; }
aws cloudfront create-invalidation --distribution-id "$DISTRIBUTION" --paths "/$PREFIX/*" \
  --query 'Invalidation.{Id:Id,Status:Status}' --output table
echo "== Done: https://moaswift.no/$PREFIX/"
