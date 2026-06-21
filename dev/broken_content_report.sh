#!/usr/bin/env bash
# Generate a creator-facing broken-content report from a regen _failures.*.tsv.
# Classifies every failure into a plain-English category + suggested action, so
# whoever deployed the entity can fix it. Read-only (no writes to the corpus).
#   usage: broken_content_report.sh <failures.tsv> <out.csv>
set -euo pipefail
TSV="${1:?failures tsv}"; OUT="${2:?out csv}"
ENVF=/home/dcl/umbrella/env/content.env
set -a; . "$ENVF"; set +a
PSQL(){ PGPASSWORD="$POSTGRES_CONTENT_PASSWORD" psql -h /home/dcl/umbrella/data/run -p 5433 \
        -U "$POSTGRES_CONTENT_USER" -d content_rust -v ON_ERROR_STOP=1 "$@"; }
tmp=$(mktemp -d)

# --- missing-dep rows: entity, glb, dep, resolved-basename ---
tail -n +2 "$TSV" | grep -aP '\tdep ".*not in entity content' \
  | sed -E 's/^([^:]+)::([^\t]*)\tdep "([^"]+)" -> "([^"]+)".*/\1\t\2\t\3\t\4/' \
  | awk -F'\t' '{n=split($4,a,"/"); print $1"\t"$2"\t"$3"\t"tolower(a[n])}' > "$tmp/md.tsv"

# classify mis-pathed (basename exists elsewhere in entity) vs absent, in one query
PSQL >/dev/null <<SQL
CREATE TEMP TABLE md(entity text, glb text, dep text, base text);
\copy md FROM '$tmp/md.tsv' WITH (FORMAT csv, DELIMITER E'\t', QUOTE E'\b')
\copy (SELECT md.entity, md.glb, CASE WHEN EXISTS(SELECT 1 FROM deployments d JOIN content_files cf ON cf.deployment=d.id WHERE d.entity_id=md.entity AND regexp_replace(lower(cf.key),'^.*/','')=md.base) THEN 'missing_texture_mispathed' ELSE 'missing_texture_never_deployed' END, md.dep FROM md) TO '$tmp/md_out.csv' WITH (FORMAT csv)
SQL

# --- assemble the full report ---
{
  echo "entity_id,asset,category,detail,suggested_action"
  # missing-dep (classified above)
  awk -F',' 'BEGIN{OFS=","}{
    cat=$3; dep=$4;
    if(cat=="missing_texture_mispathed"){act="republish: deploy the texture in the folder the glb references (kit-pack mis-path)"}
    else{act="re-upload the missing texture asset, then redeploy"}
    print $1,$2,cat,"references \""dep"\" which is not at the referenced path",act
  }' "$tmp/md_out.csv"
  # web-export junk URIs
  tail -n +2 "$TSV" | grep -a 'has a URI scheme' \
    | sed -E 's/^([^:]+)::([^\t]*)\tglTF URI "([^"]+)".*/\1,\2,broken_uri_web_export,"texture URI \3 is not entity content (web-export leftover)",re-export the model with embedded or relative textures/'
  # corrupt glb
  tail -n +2 "$TSV" | grep -aPv '\t(dep ".*not in entity content|.*has a URI scheme|panic:)' \
    | sed -E 's/^([^:]+)::([^\t]*)\t(.*)$/\1,\2,corrupt_or_invalid_glb,"\3",redeploy a valid glb (file is truncated\/not a glb\/bad JSON)/'
  # abgen bugs (panics)
  tail -n +2 "$TSV" | grep -aP '\tpanic:' \
    | sed -E 's/^([^:]+)::([^\t]*)\t(panic:[^\t]*)$/\1,\2,abgen_bug,"\3",abgen-side fix (see dev\/CONVERSION_FAILURES.md)/'
} > "$OUT"

rm -rf "$tmp"
echo "wrote $OUT  ($(($(wc -l < "$OUT")-1)) rows)"
echo "=== category counts ==="
tail -n +2 "$OUT" | cut -d',' -f3 | sort | uniq -c | sort -rn
