//! Local reference snapshots; provenance is classified per manifest entry.
pub struct Payload {
    pub key: &'static str,
    pub raw: &'static [u8],
    pub text: &'static str,
}
pub const PAYLOADS: &[Payload] = &[
    Payload {
        key: "labor-code-reference",
        raw: include_bytes!("../../../library/starter/raw/labor-code-reference.html"),
        text: include_str!("../../../library/starter/text/labor-code-reference.txt"),
    },
    Payload {
        key: "ra-11210",
        raw: include_bytes!("../../../library/starter/raw/ra-11210.html"),
        text: include_str!("../../../library/starter/text/ra-11210.txt"),
    },
    Payload {
        key: "ra-11058",
        raw: include_bytes!("../../../library/starter/raw/ra-11058.html"),
        text: include_str!("../../../library/starter/text/ra-11058.txt"),
    },
    Payload {
        key: "ra-10173",
        raw: include_bytes!("../../../library/starter/raw/ra-10173.html"),
        text: include_str!("../../../library/starter/text/ra-10173.txt"),
    },
    Payload {
        key: "npc-dpa-irr",
        raw: include_bytes!("../../../library/starter/raw/npc-dpa-irr.pdf"),
        text: include_str!("../../../library/starter/text/npc-dpa-irr.txt"),
    },
    Payload {
        key: "npc-2023-06",
        raw: include_bytes!("../../../library/starter/raw/npc-2023-06.pdf"),
        text: include_str!("../../../library/starter/text/npc-2023-06.txt"),
    },
    Payload {
        key: "ra-10963",
        raw: include_bytes!("../../../library/starter/raw/ra-10963.html"),
        text: include_str!("../../../library/starter/text/ra-10963.txt"),
    },
    Payload {
        key: "bir-rr-29-2025",
        raw: include_bytes!("../../../library/starter/raw/bir-rr-29-2025.pdf"),
        text: include_str!("../../../library/starter/text/bir-rr-29-2025.txt"),
    },
    Payload {
        key: "ra-11199",
        raw: include_bytes!("../../../library/starter/raw/ra-11199.pdf"),
        text: include_str!("../../../library/starter/text/ra-11199.txt"),
    },
    Payload {
        key: "sss-2024-006",
        raw: include_bytes!("../../../library/starter/raw/sss-2024-006.pdf"),
        text: include_str!("../../../library/starter/text/sss-2024-006.txt"),
    },
    Payload {
        key: "ra-11223",
        raw: include_bytes!("../../../library/starter/raw/ra-11223.pdf"),
        text: include_str!("../../../library/starter/text/ra-11223.txt"),
    },
    Payload {
        key: "philhealth-2025-0002",
        raw: include_bytes!("../../../library/starter/raw/philhealth-2025-0002.pdf"),
        text: include_str!("../../../library/starter/text/philhealth-2025-0002.txt"),
    },
    Payload {
        key: "philhealth-2026-0050",
        raw: include_bytes!("../../../library/starter/raw/philhealth-2026-0050.pdf"),
        text: include_str!("../../../library/starter/text/philhealth-2026-0050.txt"),
    },
    Payload {
        key: "ra-9679",
        raw: include_bytes!("../../../library/starter/raw/ra-9679.pdf"),
        text: include_str!("../../../library/starter/text/ra-9679.txt"),
    },
    Payload {
        key: "labor-code-2022-upload",
        raw: include_bytes!("../../../library/starter/raw/labor-code-2022-upload.pdf"),
        text: include_str!("../../../library/starter/text/labor-code-2022-upload.txt"),
    },
    Payload {
        key: "dole-252-25-upload",
        raw: include_bytes!("../../../library/starter/raw/dole-252-25-upload.pdf"),
        text: include_str!("../../../library/starter/text/dole-252-25-upload.txt"),
    },
    Payload {
        key: "oshs-2017-upload",
        raw: include_bytes!("../../../library/starter/raw/oshs-2017-upload.pdf"),
        text: include_str!("../../../library/starter/text/oshs-2017-upload.txt"),
    },
    Payload {
        key: "pagibig-460-upload",
        raw: include_bytes!("../../../library/starter/raw/pagibig-460-upload.pdf"),
        text: include_str!("../../../library/starter/text/pagibig-460-upload.txt"),
    },
    Payload {
        key: "pagibig-499-upload",
        raw: include_bytes!("../../../library/starter/raw/pagibig-499-upload.pdf"),
        text: include_str!("../../../library/starter/text/pagibig-499-upload.txt"),
    },
];
