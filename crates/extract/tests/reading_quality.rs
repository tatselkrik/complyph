use complyph_extract::*;
use std::{fs, path::Path};
fn sample(name: &str) -> Report {
    extract(
        if name.ends_with(".pdf") {
            Format::Pdf
        } else {
            Format::Docx
        },
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples")
                .join(name),
        )
        .unwrap(),
    )
}
fn text(report: &Report) -> String {
    report
        .units
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn supported_layouts_match_independent_text_order_unicode_and_physical_pages() {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/expected-reading-quality.json"
    ))
    .unwrap();
    for (name, case) in expected["reader_cases"].as_object().unwrap() {
        let report = sample(name);
        assert_eq!(report.status, Status::Ready, "{name}: {}", report.summary);
        assert_eq!(report.engine, ENGINE);
        assert!(report.reading.is_some());
        let text = text(&report);
        for needle in case["contains"].as_array().unwrap() {
            assert!(
                text.contains(needle.as_str().unwrap()),
                "{name} missing {needle}: {text}"
            );
        }
        for needle in case["excludes"].as_array().into_iter().flatten() {
            assert!(
                !text.contains(needle.as_str().unwrap()),
                "{name} read omitted content {needle}"
            );
        }
        if let Some(pages) = case["pages"].as_u64() {
            assert_eq!(report.units.len(), pages as usize);
            for (i, u) in report.units.iter().enumerate() {
                assert_eq!(u.location, Location::PdfPage { page: i as u32 + 1 });
            }
        }
        if let Some(order) = case["ordered"].as_array() {
            let positions = order
                .iter()
                .map(|s| text.find(s.as_str().unwrap()).unwrap())
                .collect::<Vec<_>>();
            assert!(
                positions.windows(2).all(|p| p[0] < p[1]),
                "{name}: wrong column order: {text}"
            );
        }
        if case["manual"] == false {
            assert_eq!(report.warnings.len(), 1, "{name}: {:?}", report.warnings);
        }
        if let Some(warning) = case["warning"].as_str() {
            assert!(
                report.warnings.iter().any(|w| w.contains(warning)),
                "{name} missing warning {warning}: {:?}",
                report.warnings
            );
        }
    }
}
#[test]
fn word_boxes_deduplicate_identical_alternatives_without_splitting_body_evidence_locations() {
    let report = sample("Synthetic Word text boxes.docx");
    assert_eq!(
        text(&report)
            .matches("Normal daily working hours are 9 hours.")
            .count(),
        1
    );
    let body = report
        .units
        .iter()
        .find(|u| matches!(u.location, Location::WordParagraph { paragraph: 2, .. }))
        .unwrap();
    assert!(body.text.contains("BODY-BEFORE") && body.text.contains("BODY-AFTER"));
    let term = report
        .units
        .iter()
        .find(|u| u.text.contains("Normal daily working hours"))
        .unwrap();
    assert!(
        matches!(&term.location,Location::WordTextBox{part,text_box:2,paragraph:4,table_cell:None}if part=="word/document.xml")
    );
    let cell = report
        .units
        .iter()
        .find(|u| u.text == "BOX-TABLE-PASSAGE")
        .unwrap();
    assert!(
        matches!(&cell.location,Location::WordTextBox{text_box:3,paragraph:10,table_cell:Some(cell),..}if cell.table==1&&cell.row==1&&cell.cell==1)
    );
    let header = report
        .units
        .iter()
        .find(|u| u.text == "HEADER-TEXT-BOX-PASSAGE")
        .unwrap();
    assert!(
        matches!(&header.location,Location::WordTextBox{part,text_box:1,paragraph:2,..}if part=="word/header1.xml")
    );
    let mut seen = std::collections::HashSet::new();
    for unit in &report.units {
        assert!(
            seen.insert(serde_json::to_string(&unit.location).unwrap()),
            "duplicate evidence location"
        );
    }
}
#[test]
fn missing_image_and_font_content_is_flagged_before_any_numeric_interpretation() {
    let mixed = sample("Synthetic mixed image text.pdf");
    assert!(!text(&mixed).contains("Normal daily working hours"));
    assert!(
        mixed
            .warnings
            .iter()
            .any(|w| w.contains("substantial image"))
    );
    let font = sample("Synthetic unsupported font.pdf");
    assert!(
        font.warnings.iter().any(|w| w.contains("font characters")),
        "{:?} {}",
        font.warnings,
        text(&font)
    );
    let word = sample("Synthetic Word embedded content.docx");
    for needle in [
        "Alternative Word representations",
        "Tracked changes",
        "field results",
        "Merged table",
        "embedded object",
    ] {
        assert!(
            word.warnings
                .iter()
                .any(|w| w.to_lowercase().contains(&needle.to_lowercase())),
            "missing {needle}: {:?}",
            word.warnings
        );
    }
}
#[test]
fn earlier_reading_json_keeps_its_exact_canonical_bytes_and_ocr_provenance() {
    let raw = format!(
        concat!(
            "{{\"engine\":\"complyph-ocr-1\",\"format\":\"pdf\",\"status\":\"ready\",\"summary\":\"Legacy reading.\",\"units\":[{{\"location\":{{\"type\":\"pdf_page\",\"page\":1}},\"text\":\"Legacy text.\"}}],\"warnings\":[],\"ocr\":{{\"engine_version\":\"synthetic-legacy\",\"renderer\":\"pdfium-8165\",\"language\":\"eng\",\"model_sha256\":\"{}\",\"confidence_threshold\":8000,\"pages\":[{{\"page\":1,\"words\":2,\"uncertain\":[]}}]}}}}"
        ),
        OCR_MODEL_SHA256
    );
    let report: Report = serde_json::from_str(&raw).unwrap();
    assert!(valid_ocr(&report));
    assert_eq!(serde_json::to_string(&report).unwrap(), raw);
    assert!(report.reading.is_none());
    assert!(report.ocr.unwrap().processing.is_none());
}
#[test]
fn current_ocr_requires_bounded_processing_metadata_and_options_reject_unknown_fields() {
    let mut report = Report {
        engine: OCR_ENGINE.into(),
        format: Format::Pdf,
        status: Status::Ready,
        summary: String::new(),
        units: vec![TextUnit {
            location: Location::PdfPage { page: 1 },
            text: "Synthetic words".into(),
        }],
        warnings: vec![OCR_WARNING.into()],
        reading: None,
        ocr: Some(OcrInfo {
            engine_version: "synthetic-validation".into(),
            renderer: "pdfium-8165".into(),
            language: "eng".into(),
            model_sha256: OCR_MODEL_SHA256.into(),
            confidence_threshold: 8000,
            pages: vec![OcrPage {
                page: 1,
                words: 2,
                uncertain: vec![],
                processing: Some(Default::default()),
            }],
            processing: Some(Default::default()),
        }),
    };
    assert!(valid_ocr(&report));
    report.ocr.as_mut().unwrap().pages[0]
        .processing
        .as_mut()
        .unwrap()
        .width = 2801;
    assert!(!valid_ocr(&report));
    report.ocr.as_mut().unwrap().pages[0].processing = Some(Default::default());
    report.ocr.as_mut().unwrap().processing = None;
    assert!(!valid_ocr(&report));
    for json in [
        r#"{"rotation":"clockwise_450"}"#,
        r#"{"upload":"https://example.invalid"}"#,
        r#"{"layout":"remote"}"#,
    ] {
        assert!(serde_json::from_str::<OcrSettings>(json).is_err());
    }
}

#[test]
fn filled_form_values_are_not_silently_treated_as_a_complete_plain_text_reading() {
    use lopdf::{Object, dictionary};
    let mut document = lopdf::Document::load_mem(include_bytes!(
        "../../../examples/Synthetic revision original.pdf"
    ))
    .unwrap();
    let empty = document.add_object(dictionary! {"Fields"=>Vec::<Object>::new()});
    let catalog = document
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_dictionary_mut(catalog)
        .unwrap()
        .set("AcroForm", empty);
    assert!(
        pdf_content_cautions(&document).is_empty(),
        "An empty placeholder is not an unread form value"
    );
    let widget=document.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Fictional term"),"V"=>Object::string_literal("Unestablished filled value"),"Rect"=>vec![40.into(),50.into(),400.into(),90.into()]});
    let form = document.add_object(dictionary! {"Fields"=>vec![Object::Reference(widget)]});
    let page = *document.get_pages().get(&1).unwrap();
    document
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", vec![Object::Reference(widget)]);
    let root = document
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_dictionary_mut(root)
        .unwrap()
        .set("AcroForm", form);
    let mut bytes = vec![];
    document.save_to(&mut bytes).unwrap();
    let report = extract(Format::Pdf, &bytes);
    assert_eq!(report.status, Status::Ready);
    assert!(report.warnings.iter().any(|w| w.contains("form/XFA")));
    assert!(report.warnings.iter().any(|w| w.contains("appearances")));
    assert!(!text(&report).contains("Unestablished filled value"));
}
