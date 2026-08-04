use serde::{Serialize, Deserialize};
use indexmap::IndexMap;

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub version: u32, pub blocks: Vec<Block>,
    pub images: IndexMap<String, ImageMetadata>,
    pub formulas: IndexMap<String, FormulaResource>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block { Paragraph(Paragraph), Table(Table) }

#[derive(Serialize, Deserialize, Default)]
pub struct Paragraph { pub id: String, pub text: String, pub inline_objects: Vec<InlineObject> }

#[derive(Serialize, Deserialize)]
pub struct InlineObject { pub position: usize, pub id: String, pub kind: String, pub ratio: Option<f32> }

#[derive(Serialize, Deserialize)]
pub struct Table { pub id: String, pub rows: Vec<Vec<String>> }

#[derive(Serialize, Deserialize)]
pub struct ImageMetadata { pub width: u32, pub height: u32, pub file: String }

#[derive(Serialize, Deserialize)]
pub struct FormulaResource { pub raw: String, pub kind: String }