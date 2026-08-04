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
        .map_err(|e| JsValue::from_str(&format!("Khởi tạo thất bại: {}", e)))?;

    let doc = parser.parse()
        .map_err(|e| JsValue::from_str(&format!("Phân tích thất bại: {}", e)))?;

    serde_json::to_string(&doc)
        .map_err(|e| JsValue::from_str(&format!("Lỗi chuyển đổi JSON: {}", e)))
}
