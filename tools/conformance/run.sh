#!/usr/bin/env bash
# External conformance checks on journey output (Linux CI). Usage: tools/conformance/run.sh <journey-results-dir> <out-dir>
# Each check prints PASS/FAIL; the script exits non-zero if any check fails. Results also go to <out-dir>/conformance.json.
set -u
R=${1:?journey results dir}
OUT=${2:?out dir}
mkdir -p "$OUT"
results=()
fail=0
check() {
  local name=$1; shift
  if "$@" >"$OUT/$name.log" 2>&1; then
    echo "PASS $name"; results+=("{\"check\": \"$name\", \"status\": \"pass\"}")
  else
    echo "FAIL $name (log: $OUT/$name.log)"; tail -20 "$OUT/$name.log"; results+=("{\"check\": \"$name\", \"status\": \"fail\"}"); fail=1
  fi
}

# veraPDF (PDF/UA-1) on the PDF/UA journey output and an ordinary tagged export.
verapdf_ua() {
  local f=$1
  docker run --rm -v "$(cd "$(dirname "$f")" && pwd):/data" verapdf/cli:latest --flavour ua1 --format text "/data/$(basename "$f")" | tee /dev/stderr | grep -q "^PASS"
}
check verapdf-ua1-tagged "verapdf_ua" "$R/J-AX-003/tagged.pdf"
check verapdf-ua1-plain-export "verapdf_ua" "$R/J-EX-007/plain.pdf"

# epubcheck on the EPUB export.
epubcheck_run() {
  [ -f /tmp/epubcheck/epubcheck.jar ] || {
    curl -sSL -o /tmp/epubcheck.zip https://github.com/w3c/epubcheck/releases/download/v5.1.0/epubcheck-5.1.0.zip &&
      unzip -q -o /tmp/epubcheck.zip -d /tmp && mv /tmp/epubcheck-5.1.0 /tmp/epubcheck
  }
  java -jar /tmp/epubcheck/epubcheck.jar "$1"
}
check epubcheck "epubcheck_run" "$R/J-EX-006/news.epub"

# XPS rendered by libgxps (an independent XPS implementation); the text must survive.
xps_render() {
  xpstopdf "$1" "$OUT/news-from-xps.pdf" && pdftotext "$OUT/news-from-xps.pdf" - | tee /dev/stderr | grep -q "Spring newsletter"
}
check xps-libgxps "xps_render" "$R/J-EX-006/news.xps"

# Real print hand-off through CUPS to the cups-pdf virtual printer.
real_print() {
  rm -rf "$HOME/PDF"
  NEWPUB_REAL_PRINT=1 ./target/release/newpub-journeys --root journeys/conformance --out "$OUT/print" --filter UI-PR-101 || return 1
  for _ in $(seq 1 30); do ls "$HOME"/PDF/*.pdf >/dev/null 2>&1 && break; sleep 1; done
  local f
  f=$(ls -t "$HOME"/PDF/*.pdf 2>/dev/null | head -1)
  [ -n "$f" ] && pdftotext "$f" - | tee /dev/stderr | grep -q "Printed by newpub"
}
check cups-print "real_print"

(IFS=,; echo "[${results[*]}]") >"$OUT/conformance.json"
exit $fail
