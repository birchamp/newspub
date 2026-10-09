# Publisher sample files

These `.pub` files come from Apache POI's test data (`test-data/publisher`, Apache License 2.0), commit
ae62bb5116b9aee19ebd5834e3a82066132c9f7f. Microsoft's sample templates were not copied from Publisher; these are
POI's own test files and files attached to its public bug reports (51318, 60685).

- `Sample.pub` and `Sample_2010.pub`: Publisher 2003+ and 2010. Two A4 pages, two text boxes on page 1
  (Times New Roman 10; Arial 20 bold italic) and a table and hyperlinks on page 2.
- `Sample98.pub` and `Sample2000.pub`: the same publication saved as Publisher 98 and 2000.
- `SampleNewsletter.pub`: a four-page newsletter with pictures and coloured shapes.
- `SampleBrochure.pub`: a two-page A4 landscape brochure with pictures.
- `51318.pub`, `60685.pub`: one-page US Letter files from POI bug reports.
- `fuzz-1.pub` to `fuzz-3.pub`: minimised fuzzer cases (damaged files) that must fail without crashing.

The expected positions in the J-PI journeys were measured from LibreOffice 24.2's rendering of the same files
(its Publisher import is the independent libmspub library), within a few points.
