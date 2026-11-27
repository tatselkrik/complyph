//! Versioned, local OCR options. Serialized with readings and saved reviews.
use serde::{Deserialize, Serialize};
pub const OCR_PROCESSING_PROFILE: &str = "complyph-raster-1";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrRotation {
    #[default]
    None,
    #[serde(rename = "clockwise_90")]
    Clockwise90,
    #[serde(rename = "clockwise_180")]
    Clockwise180,
    #[serde(rename = "clockwise_270")]
    Clockwise270,
}
impl OcrRotation {
    pub const ALL: [Self; 4] = [
        Self::None,
        Self::Clockwise90,
        Self::Clockwise180,
        Self::Clockwise270,
    ];
    pub fn degrees(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Clockwise90 => 90,
            Self::Clockwise180 => 180,
            Self::Clockwise270 => 270,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "No extra rotation",
            Self::Clockwise90 => "90° clockwise",
            Self::Clockwise180 => "180° clockwise",
            Self::Clockwise270 => "270° clockwise",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrLayout {
    #[default]
    Auto,
    SingleColumn,
    SparseText,
}
impl OcrLayout {
    pub const ALL: [Self; 3] = [Self::Auto, Self::SingleColumn, Self::SparseText];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic page layout",
            Self::SingleColumn => "One text block",
            Self::SparseText => "Scattered text",
        }
    }
    pub fn segmentation(self) -> i32 {
        match self {
            Self::Auto => 3,
            Self::SingleColumn => 6,
            Self::SparseText => 11,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OcrSettings {
    pub rotation: OcrRotation,
    pub layout: OcrLayout,
    pub enhance_scan: bool,
}
impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            rotation: OcrRotation::None,
            layout: OcrLayout::Auto,
            enhance_scan: true,
        }
    }
}
impl OcrSettings {
    pub fn label(&self) -> String {
        format!(
            "{} · {} · scan enhancement {}",
            self.rotation.label(),
            self.layout.label(),
            if self.enhance_scan { "on" } else { "off" }
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrProcessing {
    pub profile: String,
    pub settings: OcrSettings,
}
impl Default for OcrProcessing {
    fn default() -> Self {
        Self {
            profile: OCR_PROCESSING_PROFILE.into(),
            settings: OcrSettings::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrPageProcessing {
    pub width: u32,
    pub height: u32,
    pub deskew_hundredths: i16,
    pub contrast_stretched: bool,
    pub contrast_low: u8,
    pub contrast_high: u8,
    pub skew_limit_reached: bool,
    pub table_lines_detected: bool,
}
impl Default for OcrPageProcessing {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
            deskew_hundredths: 0,
            contrast_stretched: false,
            contrast_low: 0,
            contrast_high: 255,
            skew_limit_reached: false,
            table_lines_detected: false,
        }
    }
}
impl OcrPageProcessing {
    pub fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 2800
            && self.height <= 2800
            && (-300..=300).contains(&self.deskew_hundredths)
            && self.contrast_low < self.contrast_high
    }
    pub fn label(&self) -> String {
        format!(
            "{} × {} pixels · straightening {:.2}° · contrast {}",
            self.width,
            self.height,
            f64::from(self.deskew_hundredths) / 100.0,
            if self.contrast_stretched {
                "enhanced"
            } else {
                "unchanged"
            }
        )
    }
}
