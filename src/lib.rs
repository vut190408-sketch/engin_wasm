mod models;
mod relationships;
mod styles;
mod numbering;
mod parser;

use wasm_bindgen::prelude::*;
use crate::parser::DocxParser;

#[wasm_bindgen]
pub fn parse_docx_to_json(file_bytes: &[u8]) -> Result<String, JsValue> {
    let mut parser = DocxParser::new(file_bytes)
        .map_err(|e| JsValue::from_str(&format!("Init Error: {}", e)))?;
    
    let doc = parser.parse()
        .map_err(|e| JsValue::from_str(&format!("Parse Error: {}", e)))?;
    
    serde_json::to_string(&doc)
        .map_err(|e| JsValue::from_str(&format!("JSON Error: {}", e)))
}
