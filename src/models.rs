use serde::{Serialize, Deserialize};
use indexmap::IndexMap;

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub blocks: Vec<Block>,
    pub images: IndexMap<String, ImageMetadata>,
    pub formulas: IndexMap<String, FormulaResource>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Paragraph(Paragraph),
    Heading(Heading),
    Table(Table),
    Image(ImageBlock),
    PageBreak,
    SectionBreak,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Paragraph {
    pub id: String,
    pub text: String,
    pub styles: StyleMap,
    #[serde(rename = "inlineObjects")]
    pub inline_objects: Vec<InlineObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbering: Option<NumberingMetadata>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Heading {
    pub id: String,
    pub level: u8,
    pub text: String,
    pub styles: StyleMap,
    #[serde(rename = "inlineObjects")]
    pub inline_objects: Vec<InlineObject>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct StyleMap {
    #[serde(skip_serializing_if = "Vec::is_empty")] pub bold: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub italic: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub underline: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub strike: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub superscript: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub subscript: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub color: Vec<ValueRange>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub highlight: Vec<ValueRange>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub font_size: Vec<ValueRange>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub font_family: Vec<ValueRange>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Range { pub start: usize, pub end: usize }

#[derive(Serialize, Deserialize, Clone)]
pub struct ValueRange { pub start: usize, pub end: usize, pub value: String }

#[derive(Serialize, Deserialize, Clone)]
pub struct InlineObject {
    pub position: usize,
    pub id: String,
    pub kind: String, // "formula", "image"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Table {
    pub id: String,
    pub rows: Vec<Vec<TableCell>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TableCell {
    #[serde(rename = "rowSpan")] pub row_span: u32,
    #[serde(rename = "colSpan")] pub col_span: u32,
    pub blocks: Vec<Block>, // Bảng hỗ trợ lồng Block bên trong ô (Cell)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ImageBlock { pub id: String }

#[derive(Serialize, Deserialize, Clone)]
pub struct ImageMetadata {
    pub file: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    #[serde(rename = "aspectRatio")]
    pub aspect_ratio: f32,
    pub display: String, // "inline", "block", "floating"
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FormulaResource { pub raw: String, pub kind: String }

#[derive(Serialize, Deserialize, Clone)]
pub struct NumberingMetadata { pub kind: String, pub level: u32 }
