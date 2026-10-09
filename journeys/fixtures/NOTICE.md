# Fixture provenance

- `tika-sample.pub` is `testPUBLISHER.pub` from the Apache Tika test corpus (Apache License 2.0;
  https://github.com/apache/tika). It is a test document written for Tika, not Microsoft content.
- Every other fixture was generated for this project (see PROGRESS.md) and is original.
- `shapes.emf` and `shapes.wmf` are written by `tools/fixtures/make_metafiles.py` from the published MS-EMF / MS-WMF record layouts.
- `flipped-window.wmf` is a clip-art picture embedded in Apache POI's `SampleBrochure.pub` test file (Apache License
  2.0), extracted unchanged: a WMF with no placeable header and a negative window height (a flipped y axis).
- `pub/*.pub` are Apache POI test files; see `pub/README.md`.
