use wasm_bindgen::prelude::*;
use std::io::{Cursor, Read};
use zip::ZipArchive;
use serde::{Serialize, Deserialize};
use indexmap::IndexMap;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

// --- 1. ĐỊNH NGHĨA CẤU TRÚC JSON ĐẦU RA (THEO SPEC) ---

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block {
    Paragraph(Paragraph),
    Heading(Paragraph),
    Table(Table),
    Image(ImageBlock),
    PageBreak,
    SectionBreak,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Paragraph {
    pub text: String,
    pub styles: Styles,
    #[serde(rename = "inlineObjects")]
    pub inline_objects: Vec<InlineObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbering: Option<NumberingMetadata>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Styles {
    #[serde(skip_serializing_if = "Vec::is_empty")] pub bold: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub italic: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub underline: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub strike: Vec<Range>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub color: Vec<ColorRange>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub highlight: Vec<ColorRange>,
}

#[derive(Serialize, Deserialize)]
pub struct Range { pub start: usize, pub end: usize }

#[derive(Serialize, Deserialize)]
pub struct ColorRange { pub start: usize, pub end: usize, pub value: String }

#[derive(Serialize, Deserialize)]
pub struct InlineObject {
    pub position: usize,
    pub id: String,
    pub kind: String, // "formula", "image", "vector"
}

#[derive(Serialize, Deserialize)]
pub struct ImageBlock { pub id: String }

#[derive(Serialize, Deserialize)]
pub struct Table {
    pub rows: Vec<Vec<TableCell>>,
}

#[derive(Serialize, Deserialize)]
pub struct TableCell {
    #[serde(rename = "rowSpan")] pub row_span: u32,
    #[serde(rename = "colSpan")] pub col_span: u32,
    pub blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize)]
pub struct ImageMetadata {
    pub file: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: f32,
    pub display: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct NumberingMetadata {
    pub kind: String,
    pub level: u32,
}

#[derive(Serialize, Deserialize, Default)]
pub struct OutputJson {
    pub version: u32,
    pub blocks: Vec<Block>,
    pub images: IndexMap<String, ImageMetadata>,
    pub formulas: IndexMap<String, String>,
}

// --- 2. LOGIC TRÍCH XUẤT CHÍNH ---

#[wasm_bindgen]
pub fn parse_docx_to_json(file_bytes: &[u8]) -> Result<String, JsValue> {
    let reader = Cursor::new(file_bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut output = OutputJson {
        version: 1,
        ..Default::default()
    };

    // Bước 1: Lưu trữ các tệp media (ảnh) dưới dạng Base64 để hiển thị trên Web
    // Trong thực tế cậu có thể tách riêng thư mục media/, nhưng ở đây tớ nhét vào JSON cho tiện
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("word/media/") {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();
            let b64 = format!("data:image/png;base64,{}", base64::encode(&buffer));
            let id = name.replace("word/media/", "").replace(".", "_");
            output.images.insert(id, ImageMetadata {
                file: name,
                mime: "image/png".into(),
                width: 0, height: 0, aspect_ratio: 1.0,
                display: "inline".into(),
            });
        }
    }

    // Bước 2: Đọc document.xml
    let mut doc_xml = String::new();
    archive.by_name("word/document.xml")
        .map_err(|_| JsValue::from_str("Không tìm thấy document.xml"))?
        .read_to_string(&mut doc_xml).unwrap();

    let mut reader = Reader::from_str(&doc_xml);
    reader.trim_text(false);
    
    let mut buf = Vec::new();
    let mut blocks = Vec::new();

    // Duyệt XML
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:p" => {
                blocks.push(Block::Paragraph(parse_p(&mut reader, &mut output)));
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:tbl" => {
                blocks.push(Block::Table(parse_tbl(&mut reader, &mut output)));
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    output.blocks = blocks;
    serde_json::to_string(&output).map_err(|e| JsValue::from_str(&e.to_string()))
}

// --- 3. HÀM PHỤ TRỢ PARSE ĐOẠN VĂN (w:p) ---
fn parse_p(reader: &mut Reader<&[u8]>, out: &mut OutputJson) -> Paragraph {
    let mut p = Paragraph::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"w:r" => {
                        let (text, styles, objects) = parse_run(reader, p.text.chars().count(), out);
                        p.text.push_str(&text);
                        // Gộp styles (đơn giản hóa)
                        if styles.bold { p.styles.bold.push(Range { start: styles.start, end: styles.end }); }
                        if styles.italic { p.styles.italic.push(Range { start: styles.start, end: styles.end }); }
                        for obj in objects { p.inline_objects.push(obj); }
                    }
                    b"m:oMath" => {
                        let math_id = format!("formula_{:03}", out.formulas.len() + 1);
                        let raw_xml = "OMML_RAW_CONTENT".to_string(); // Giả lập lấy nội dung XML
                        out.formulas.insert(math_id.clone(), raw_xml);
                        p.inline_objects.push(InlineObject {
                            position: p.text.chars().count(),
                            id: math_id,
                            kind: "formula".into(),
                        });
                    }
                    _ => (),
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"w:p" => break,
            _ => (),
        }
        buf.clear();
    }
    p
}

// --- 4. HÀM PHỤ TRỢ PARSE RUN (w:r) ---
struct TempStyles { start: usize, end: usize, bold: bool, italic: bool }

fn parse_run(reader: &mut Reader<&[u8]>, start_pos: usize, out: &mut OutputJson) -> (String, TempStyles, Vec<InlineObject>) {
    let mut text = String::new();
    let mut is_bold = false;
    let mut is_italic = false;
    let mut objects = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"w:b" => is_bold = true,
                b"w:i" => is_italic = true,
                b"w:drawing" => {
                    let img_id = format!("image_{:03}", out.images.len());
                    objects.push(InlineObject { position: start_pos, id: img_id, kind: "image".into() });
                }
                _ => (),
            },
            Ok(Event::Text(e)) => {
                text.push_str(&e.unescape().unwrap());
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"w:r" => break,
            _ => (),
        }
        buf.clear();
    }
    let end_pos = start_pos + text.chars().count();
    (text, TempStyles { start: start_pos, end: end_pos, bold: is_bold, italic: is_italic }, objects)
}

// --- 5. HÀM PHỤ TRỢ PARSE BẢNG (w:tbl) ---
fn parse_tbl(reader: &mut Reader<&[u8]>, out: &mut OutputJson) -> Table {
    let mut table = Table { rows: Vec::new() };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:tr" => {
                table.rows.push(parse_tr(reader, out));
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tbl" => break,
            _ => (),
        }
        buf.clear();
    }
    table
}

fn parse_tr(reader: &mut Reader<&[u8]>, out: &mut OutputJson) -> Vec<TableCell> {
    let mut cells = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:tc" => {
                cells.push(parse_tc(reader, out));
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tr" => break,
            _ => (),
        }
        buf.clear();
    }
    cells
}

fn parse_tc(reader: &mut Reader<&[u8]>, out: &mut OutputJson) -> TableCell {
    let mut cell = TableCell { row_span: 1, col_span: 1, blocks: Vec::new() };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"w:p" => cell.blocks.push(Block::Paragraph(parse_p(reader, out))),
                b"w:tbl" => cell.blocks.push(Block::Table(parse_tbl(reader, out))),
                _ => (),
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tc" => break,
            _ => (),
        }
        buf.clear();
    }
    cell
}
