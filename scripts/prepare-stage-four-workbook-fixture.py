"""Remove personal metadata from an explicitly marked, synthetic Office fixture.

This is test preparation, not a registered XLSX generation/editing capability.
The resulting fixture must pass the native independent verifier and real Excel.
"""

import argparse
import pathlib
import re
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=pathlib.Path)
    parser.add_argument("destination", type=pathlib.Path)
    args = parser.parse_args()
    if not args.source.is_absolute() or not args.destination.is_absolute():
        parser.error("Absolute fixture paths required")
    if not (args.source.parent / ".custodian-fixture-root").is_file():
        parser.error("Source must be in a marked synthetic fixture directory")
    if args.source.is_symlink() or args.source.stat().st_size > 8 * 1024 * 1024:
        parser.error("Ordinary bounded synthetic fixture required")
    if args.destination.exists() or not args.destination.parent.is_dir():
        parser.error("New output in an existing fixture directory required")
    with zipfile.ZipFile(args.source) as original:
        entries = original.infolist()
        if len(entries) > 32 or sum(e.file_size for e in entries) > 8 * 1024 * 1024:
            parser.error("Fixture archive limits exceeded")
        parts = []
        for entry in entries:
            data = original.read(entry)
            if entry.filename == "docProps/core.xml":
                data = (
                    b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                    b'<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/'
                    b'package/2006/metadata/core-properties" '
                    b'xmlns:dc="http://purl.org/dc/elements/1.1/">'
                    b"<dc:creator>Custodian synthetic fixture</dc:creator>"
                    b"<cp:lastModifiedBy>Custodian synthetic fixture</cp:lastModifiedBy>"
                    b"</cp:coreProperties>"
                )
            elif entry.filename == "docProps/app.xml":
                data = re.sub(rb"<(Company|Manager)>.*?</\1>", rb"<\1></\1>", data)
            elif entry.filename == "xl/workbook.xml":
                data = re.sub(
                    rb"<([A-Za-z_][\w.-]*:absPath)\b[^>]*/>",
                    lambda match: b"<" + match[1] + b' url="C:\\CustodianSyntheticFixture\\"/>',
                    data,
                )
            if b"paagu" in data.lower():
                parser.error("Fixture still contains local profile metadata")
            parts.append((entry.filename, data))
    with args.destination.open("xb") as output, zipfile.ZipFile(output, "w") as sanitized:
        for name, data in parts:
            # Rebuild neutral ZIP metadata, retaining exact worksheet cell bytes.
            sanitized.writestr(zipfile.ZipInfo(name), data)
    print("Synthetic workbook metadata sanitized; native and real-app verification still required.")


if __name__ == "__main__":
    main()
