use complyph_extract::{Format, Location, MAX_INPUT_BYTES, Status, extract};
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn examples() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

#[test]
fn all_synthetic_documents_match_independent_expected_results() {
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(examples().join("expected-results.json")).unwrap())
            .unwrap();
    for (name, expected) in expected.as_object().unwrap() {
        let bytes = fs::read(examples().join(name)).unwrap();
        let before = bytes.clone();
        let format = if name.ends_with(".pdf") {
            Format::Pdf
        } else {
            Format::Docx
        };
        let report = extract(format, &bytes);
        assert_eq!(
            serde_json::to_value(report.status).unwrap(),
            expected["status"],
            "{name}: {report:?}"
        );
        if let Some(count) = expected["units"].as_u64() {
            assert_eq!(report.units.len() as u64, count, "{name}");
        }
        let text = report
            .units
            .iter()
            .map(|unit| unit.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in expected["contains"].as_array().into_iter().flatten() {
            assert!(
                text.contains(needle.as_str().unwrap()),
                "{name} missing {needle}: {text}"
            );
        }
        for needle in expected["excludes"].as_array().into_iter().flatten() {
            assert!(
                !text.contains(needle.as_str().unwrap()),
                "{name} contains forbidden {needle}"
            );
        }
        assert_eq!(bytes, before, "{name} input changed");
    }
}

#[test]
fn pdf_pages_and_word_cells_refer_to_original_locations() {
    let pdf = extract(
        Format::Pdf,
        &fs::read(examples().join("Synthetic employment.pdf")).unwrap(),
    );
    assert_eq!(pdf.units[1].location, Location::PdfPage { page: 2 });
    assert!(pdf.units[1].text.contains("PAGE-TWO-TERMS"));
    let mixed = extract(
        Format::Pdf,
        &fs::read(examples().join("Synthetic mixed pages.pdf")).unwrap(),
    );
    assert!(mixed.units[1].text.is_empty());
    assert!(
        mixed
            .warnings
            .iter()
            .any(|warning| warning.contains("page(s) 2"))
    );
    let word = extract(
        Format::Docx,
        &fs::read(examples().join("Synthetic workplace policy.docx")).unwrap(),
    );
    let cell = word
        .units
        .iter()
        .find(|unit| unit.text == "DETAIL-CELL")
        .unwrap();
    assert!(
        matches!(&cell.location, Location::WordParagraph { part, paragraph: 6, table_cell: Some(cell) }
        if part == "word/document.xml" && cell.table == 1 && cell.row == 1 && cell.cell == 2)
    );
    let header = word
        .units
        .iter()
        .find(|unit| unit.text.starts_with("SYNTHETIC-HEADER"))
        .unwrap();
    assert!(
        matches!(&header.location, Location::WordParagraph { part, paragraph: 1, table_cell: None } if part == "word/header1.xml")
    );
}

fn docx(xml: &[u8]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("[Content_Types].xml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"<Types/>").unwrap();
    zip.start_file("word/document.xml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(xml).unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn omitted_revisions_do_not_renumber_later_original_locations() {
    let xml = "<w:document xmlns:w='http://schemas.openxmlformats.org/wordprocessingml/2006/main'><w:body>
        <w:del><w:p><w:r><w:t>Omitted paragraph</w:t></w:r></w:p>
        <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Omitted table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:del>
        <w:p><w:r><w:t>Visible paragraph</w:t></w:r></w:p>
        <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Visible table</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
        </w:body></w:document>";
    let report = extract(Format::Docx, &docx(xml.as_bytes()));
    assert_eq!(report.status, Status::Ready);
    assert_eq!(report.units.len(), 2);
    assert!(matches!(
        report.units[0].location,
        Location::WordParagraph { paragraph: 3, .. }
    ));
    assert!(
        matches!(&report.units[1].location, Location::WordParagraph { paragraph: 4, table_cell: Some(cell), .. } if cell.table == 2)
    );
}

#[test]
fn malformed_xml_entities_and_excessive_expansion_are_rejected() {
    for xml in [
        "<w:document xmlns:w='http://schemas.openxmlformats.org/wordprocessingml/2006/main'><w:p><w:r><w:t>broken</w:p>",
        "<!DOCTYPE test [<!ENTITY secret SYSTEM 'file:///never-read-private-files'>]><w:document xmlns:w='http://schemas.openxmlformats.org/wordprocessingml/2006/main'><w:p><w:r><w:t>&secret;</w:t></w:r></w:p></w:document>",
    ] {
        assert_eq!(
            extract(Format::Docx, &docx(xml.as_bytes())).status,
            Status::Unreadable
        );
    }
    let oversized = vec![b' '; 16 * 1024 * 1024 + 1];
    assert_eq!(
        extract(Format::Docx, &docx(&oversized)).status,
        Status::LimitExceeded
    );
    assert_eq!(
        extract(Format::Pdf, &vec![0; MAX_INPUT_BYTES + 1]).status,
        Status::LimitExceeded
    );
}

#[test]
fn strict_namespace_and_entities_preserve_actual_text() {
    let xml = "<q:document xmlns:q='http://purl.oclc.org/ooxml/wordprocessingml/main'><q:body><q:p><q:r><q:t>Research &amp; development &#x20B1; &#233;</q:t></q:r></q:p></q:body></q:document>";
    let report = extract(Format::Docx, &docx(xml.as_bytes()));
    assert_eq!(report.status, Status::Ready);
    assert_eq!(report.units[0].text, "Research & development ₱ é");
}
