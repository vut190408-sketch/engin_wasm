use wasm_bindgen::prelude::*;
use std::io::{Cursor, Read};
use zip::ZipArchive;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use quick_xml::events::Event;
use quick_xml::Reader;

// Cấu trúc theo spec của cậu
#[derive(Serialize, Deserialize, Default)]
pub struct Block {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    pub t: String,
}

#[derive(Serialize, Deserialize)]
pub struct ImageResource {
    pub r: f32,
    pub s: String,
    pub b64: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Resources {
    pub images: HashMap<String, ImageResource>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Document {
    #[serde(rename = "docId")]
    pub doc_id: String,
    pub blocks: Vec<Block>,
    pub resources: Resources,
}

#[wasm_bindgen]
pub fn parse_docx_to_json(file_bytes: &[u8]) -> Result<String, JsValue> {
    // 1. Mở file DOCX trên RAM
    let reader = Cursor::new(file_bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| JsValue::from_str(&e.to_string()))?;
    
    let mut doc = Document {
        doc_id: "doc_from_wasm".to_string(),
        blocks: Vec::new(),
        resources: Resources::default(),
    };

    let mut document_xml_content = String::new();

    // 2. Quét toàn bộ file trong ZIP
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();

        // NẾU LÀ ẢNH: Đọc và chuyển sang Base64
        if name.starts_with("word/media/") {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();
            
            let img_ext = name.split('.').last().unwrap_or("png");
            let base64_str = base64::encode(&buffer);
            let b64_data_uri = format!("data:image/{};base64,{}", img_ext, base64_str);
            
            // Lưu vào resources
            let img_id = name.replace("word/media/", "").replace(".", "_");
            doc.resources.images.insert(img_id, ImageResource {
                r: 1.0,
                s: "m".to_string(),
                b64: b64_data_uri,
            });
        }

        // NẾU LÀ TEXT XML: Lưu lại để xử lý sau
        if name == "word/document.xml" {
            file.read_to_string(&mut document_xml_content).unwrap();
        }
    }

    // 3. Đọc XML để bóc tách text
    let mut xml_reader = Reader::from_str(&document_xml_content);
    xml_reader.trim_text(true);

    let mut current_block_text = String::new();
    let mut is_in_text_tag = false;
    let mut block_counter = 1;

    loop {
        match xml_reader.read_event() {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"w:t" => is_in_text_tag = true,
                _ => (),
            },
            Ok(Event::Text(e)) => {
                if is_in_text_tag {
                    let text = e.unescape().unwrap().to_string();
                    current_block_text.push_str(&text);
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"w:t" => is_in_text_tag = false,
                b"w:p" => {
                    // Kết thúc 1 đoạn văn (Paragraph) -> Tạo 1 Block mới
                    if !current_block_text.trim().is_empty() {
                        doc.blocks.push(Block {
                            id: format!("b{}", block_counter),
                            block_type: "text".to_string(),
                            t: current_block_text.clone(),
                        });
                        block_counter += 1;
                    }
                    current_block_text.clear();
                },
                _ => (),
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(JsValue::from_str(&format!("XML Error: {:?}", e))),
            _ => (),
        }
    }

    // 4. Trả về JSON
    let json_str = serde_json::to_string(&doc).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(json_str)
}
