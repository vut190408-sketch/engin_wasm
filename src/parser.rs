use crate::models::*;
use crate::relationships::Relationships;
use crate::styles::Styles;
use crate::numbering::Numbering;
use std::io::{Cursor, Read};
use zip::ZipArchive;
use quick_xml::events::Event;
use quick_xml::Reader;
use uuid::Uuid;

pub struct DocxParser {
    archive: ZipArchive<Cursor<Vec<u8>>>,
    rels: Relationships,
    styles: Styles,
    numbering: Numbering,
}

#[derive(Default)]
struct RunFormat {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    superscript: bool,
    subscript: bool,
    color: Option<String>,
    highlight: Option<String>,
    font_size: Option<String>,
    font_family: Option<String>,
}

impl DocxParser {
    pub fn new(file_bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut archive = ZipArchive::new(Cursor::new(file_bytes.to_vec()))?;
        
        let rels_xml = Self::read_zip_file(&mut archive, "word/_rels/document.xml.rels").unwrap_or_default();
        let styles_xml = Self::read_zip_file(&mut archive, "word/styles.xml").unwrap_or_default();
        let numbering_xml = Self::read_zip_file(&mut archive, "word/numbering.xml").unwrap_or_default();

        Ok(Self {
            rels: Relationships::parse(&rels_xml),
            styles: Styles::parse(&styles_xml),
            numbering: Numbering::parse(&numbering_xml),
            archive,
        })
    }

    fn read_zip_file(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Option<String> {
        let mut s = String::new();
        if let Ok(mut f) = archive.by_name(name) {
            if f.read_to_string(&mut s).is_ok() {
                return Some(s);
            }
        }
        None
    }

    pub fn parse(&mut self) -> Result<Document, Box<dyn std::error::Error>> {
        let mut doc = Document {
            version: 1,
            blocks: Vec::new(),
            images: Default::default(),
            formulas: Default::default(),
        };

        let xml = Self::read_zip_file(&mut self.archive, "word/document.xml")
            .ok_or("Không tìm thấy file word/document.xml")?;

        let mut reader = Reader::from_str(&xml);
        reader.trim_text(false);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"w:p" => {
                        if let Some(block) = self.parse_paragraph_or_heading(&mut reader, &mut doc)? {
                            doc.blocks.push(block);
                        }
                    }
                    b"w:tbl" => {
                        let table = self.parse_table(&mut reader, &mut doc)?;
                        doc.blocks.push(Block::Table(table));
                    }
                    _ => (),
                },
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }

        Ok(doc)
    }

    // --- PARSE PARAGRAPH / HEADING / BREAKS ---
    fn parse_paragraph_or_heading(
        &mut self,
        reader: &mut Reader<&[u8]>,
        doc: &mut Document,
    ) -> Result<Option<Block>, Box<dyn std::error::Error>> {
        let mut text_acc = String::new();
        let mut style_map = StyleMap::default();
        let mut inline_objects = Vec::new();
        let mut numbering_meta: Option<NumberingMetadata> = None;
        let mut heading_level: Option<u8> = None;
        let mut is_page_break = false;
        let mut is_section_break = false;

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"w:pPr" => {
                        let (h_lvl, num_meta) = self.parse_paragraph_properties(reader)?;
                        heading_level = h_lvl;
                        numbering_meta = num_meta;
                    }
                    b"w:r" => {
                        self.parse_run(reader, &mut text_acc, &mut style_map, &mut inline_objects, doc)?;
                    }
                    b"w:hyperlink" => {
                        self.parse_hyperlink(reader, &mut text_acc, &mut style_map, &mut inline_objects, doc)?;
                    }
                    b"m:oMath" => {
                        let raw_omml = self.capture_xml(reader, "m:oMath")?;
                        let f_id = format!("formula_{}", Uuid::new_v4());
                        doc.formulas.insert(f_id.clone(), FormulaResource { raw: raw_omml, kind: "omml".to_string() });
                        
                        inline_objects.push(InlineObject {
                            position: text_acc.chars().count(),
                            id: f_id,
                            kind: "formula".to_string(),
                            ratio: None,
                        });
                    }
                    b"w:br" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:type" && attr.value.as_ref() == b"page" {
                                is_page_break = true;
                            }
                        }
                    }
                    b"w:sectPr" => {
                        is_section_break = true;
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:p" => break,
                _ => (),
            }
            buf.clear();
        }

        if is_page_break {
            return Ok(Some(Block::PageBreak));
        }
        if is_section_break && text_acc.trim().is_empty() {
            return Ok(Some(Block::SectionBreak));
        }

        let block_id = format!("p_{}", Uuid::new_v4());

        if let Some(level) = heading_level {
            Ok(Some(Block::Heading(Heading {
                id: format!("h_{}", Uuid::new_v4()),
                level,
                text: text_acc,
                styles: style_map,
                inline_objects,
            })))
        } else {
            Ok(Some(Block::Paragraph(Paragraph {
                id: block_id,
                text: text_acc,
                styles: style_map,
                inline_objects,
                numbering: numbering_meta,
            })))
        }
    }

    // --- PARSE PROPERTY CỦA PARAGRAPH (Heading & Numbering) ---
    fn parse_paragraph_properties(
        &self,
        reader: &mut Reader<&[u8]>,
    ) -> Result<(Option<u8>, Option<NumberingMetadata>), Box<dyn std::error::Error>> {
        let mut heading_lvl = None;
        let mut num_id = String::new();
        let mut ilvl = 0u32;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                    b"w:pStyle" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let style_val = String::from_utf8_lossy(&attr.value);
                                heading_lvl = self.styles.get_heading_level(&style_val);
                            }
                        }
                    }
                    b"w:numId" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                num_id = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    b"w:ilvl" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                if let Ok(val) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                    ilvl = val;
                                }
                            }
                        }
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:pPr" => break,
                _ => (),
            }
            buf.clear();
        }

        let num_meta = if !num_id.is_empty() {
            self.numbering.get_metadata(&num_id, ilvl)
        } else {
            None
        };

        Ok((heading_lvl, num_meta))
    }

    // --- PARSE RUN (w:r) VÀ CÁC THUỘC TÍNH DINH DANG ---
    fn parse_run(
        &mut self,
        reader: &mut Reader<&[u8]>,
        text_acc: &mut String,
        styles: &mut StyleMap,
        inline_objects: &mut Vec<InlineObject>,
        doc: &mut Document,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start_offset = text_acc.chars().count();
        let mut fmt = RunFormat::default();
        let mut run_text = String::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"w:rPr" => fmt = self.parse_run_properties(reader)?,
                    b"w:t" => {
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            run_text.push_str(&t.unescape()?);
                        }
                    }
                    b"w:tab" => run_text.push('\t'),
                    b"w:br" => run_text.push('\n'),
                    b"w:drawing" => {
                        self.parse_drawing(reader, text_acc, inline_objects, doc)?;
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:r" => break,
                _ => (),
            }
            buf.clear();
        }

        let len = run_text.chars().count();
        if len > 0 {
            let end_offset = start_offset + len;
            text_acc.push_str(&run_text);
            Self::apply_run_styles(styles, &fmt, start_offset, end_offset);
        }

        Ok(())
    }

    // --- PARSE HYPERLINK (w:hyperlink) ---
    fn parse_hyperlink(
        &mut self,
        reader: &mut Reader<&[u8]>,
        text_acc: &mut String,
        styles: &mut StyleMap,
        inline_objects: &mut Vec<InlineObject>,
        doc: &mut Document,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:r" => {
                    self.parse_run(reader, text_acc, styles, inline_objects, doc)?;
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:hyperlink" => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(())
    }

    // --- PARSE RUN PROPERTIES (10 loại Styles) ---
    fn parse_run_properties(&self, reader: &mut Reader<&[u8]>) -> Result<RunFormat, Box<dyn std::error::Error>> {
        let mut fmt = RunFormat::default();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                    b"w:b" | b"w:bCs" => fmt.bold = true,
                    b"w:i" | b"w:iCs" => fmt.italic = true,
                    b"w:u" => fmt.underline = true,
                    b"w:strike" => fmt.strike = true,
                    b"w:vertAlign" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let v = String::from_utf8_lossy(&attr.value);
                                if v == "superscript" { fmt.superscript = true; }
                                if v == "subscript" { fmt.subscript = true; }
                            }
                        }
                    }
                    b"w:color" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                fmt.color = Some(format!("#{}", String::from_utf8_lossy(&attr.value)));
                            }
                        }
                    }
                    b"w:highlight" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                fmt.highlight = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    b"w:sz" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                if let Ok(val) = String::from_utf8_lossy(&attr.value).parse::<f32>() {
                                    fmt.font_size = Some(format!("{}pt", val / 2.0));
                                }
                            }
                        }
                    }
                    b"w:rFonts" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:ascii" {
                                fmt.font_family = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:rPr" => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(fmt)
    }

    // --- GỘP CÁC RANGE LIÊN TIẾP ĐỂ TỐI ƯU JSON ---
    fn apply_run_styles(styles: &mut StyleMap, fmt: &RunFormat, start: usize, end: usize) {
        let add_range = |ranges: &mut Vec<Range>| {
            if let Some(last) = ranges.last_mut() {
                if last.end == start { last.end = end; return; }
            }
            ranges.push(Range { start, end });
        };

        let add_val_range = |ranges: &mut Vec<ValueRange>, val: String| {
            if let Some(last) = ranges.last_mut() {
                if last.end == start && last.value == val { last.end = end; return; }
            }
            ranges.push(ValueRange { start, end, value: val });
        };

        if fmt.bold { add_range(&mut styles.bold); }
        if fmt.italic { add_range(&mut styles.italic); }
        if fmt.underline { add_range(&mut styles.underline); }
        if fmt.strike { add_range(&mut styles.strike); }
        if fmt.superscript { add_range(&mut styles.superscript); }
        if fmt.subscript { add_range(&mut styles.subscript); }
        if let Some(ref c) = fmt.color { add_val_range(&mut styles.color, c.clone()); }
        if let Some(ref h) = fmt.highlight { add_val_range(&mut styles.highlight, h.clone()); }
        if let Some(ref fs) = fmt.font_size { add_val_range(&mut styles.font_size, fs.clone()); }
        if let Some(ref ff) = fmt.font_family { add_val_range(&mut styles.font_family, ff.clone()); }
    }

    // --- PARSE DRAWING (Anh inline vs Anchor / Floating) ---
    fn parse_drawing(
        &mut self,
        reader: &mut Reader<&[u8]>,
        text_acc: &String,
        inline_objects: &mut Vec<InlineObject>,
        doc: &mut Document,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut is_inline = true;
        let mut cx = 0u32;
        let mut cy = 0u32;
        let mut embed_rid = String::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                    b"wp:anchor" => is_inline = false,
                    b"wp:inline" => is_inline = true,
                    b"wp:extent" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"cx" => cx = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0),
                                b"cy" => cy = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0),
                                _ => (),
                            }
                        }
                    }
                    b"a:blip" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"r:embed" {
                                embed_rid = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:drawing" => break,
                _ => (),
            }
            buf.clear();
        }

        if !embed_rid.is_empty() {
            if let Some(target_file) = self.rels.get_target(&embed_rid).cloned() {
                let img_id = format!("image_{}", Uuid::new_v4());
                let width_px = cx / 9525; // Chuyển đổi từ EMU sang Pixel (96 DPI)
                let height_px = cy / 9525;
                let aspect_ratio = if height_px > 0 { width_px as f32 / height_px as f32 } else { 1.0 };
                
                let mime = if target_file.ends_with(".png") { "image/png" }
                           else if target_file.ends_with(".jpg") || target_file.ends_with(".jpeg") { "image/jpeg" }
                           else if target_file.ends_with(".svg") { "image/svg+xml" }
                           else { "image/unknown" };

                let display_mode = if is_inline { "inline" } else { "floating" };

                doc.images.insert(img_id.clone(), ImageMetadata {
                    file: target_file,
                    mime: mime.to_string(),
                    width: width_px,
                    height: height_px,
                    aspect_ratio,
                    display: display_mode.to_string(),
                });

                if is_inline {
                    inline_objects.push(InlineObject {
                        position: text_acc.chars().count(),
                        id: img_id,
                        kind: "image".to_string(),
                        ratio: Some(aspect_ratio),
                    });
                } else {
                    doc.blocks.push(Block::Image(ImageBlock { id: img_id }));
                }
            }
        }

        Ok(())
    }

    // --- PARSE TABLE (Bảng, rowSpan & colSpan) ---
    fn parse_table(
        &mut self,
        reader: &mut Reader<&[u8]>,
        doc: &mut Document,
    ) -> Result<Table, Box<dyn std::error::Error>> {
        let mut rows = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:tr" => {
                    let row = self.parse_table_row(reader, doc)?;
                    rows.push(row);
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tbl" => break,
                _ => (),
            }
            buf.clear();
        }

        Ok(Table {
            id: format!("table_{}", Uuid::new_v4()),
            rows,
        })
    }

    fn parse_table_row(
        &mut self,
        reader: &mut Reader<&[u8]>,
        doc: &mut Document,
    ) -> Result<Vec<TableCell>, Box<dyn std::error::Error>> {
        let mut cells = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:tc" => {
                    let cell = self.parse_table_cell(reader, doc)?;
                    cells.push(cell);
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tr" => break,
                _ => (),
            }
            buf.clear();
        }

        Ok(cells)
    }

    fn parse_table_cell(
        &mut self,
        reader: &mut Reader<&[u8]>,
        doc: &mut Document,
    ) -> Result<TableCell, Box<dyn std::error::Error>> {
        let mut col_span = 1u32;
        let mut row_span = 1u32;
        let mut cell_blocks = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"w:gridSpan" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                col_span = String::from_utf8_lossy(&attr.value).parse().unwrap_or(1);
                            }
                        }
                    }
                    b"w:vMerge" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" && attr.value.as_ref() == b"restart" {
                                row_span = 1; // Khởi tạo ô bắt đầu gộp hàng
                            }
                        }
                    }
                    b"w:p" => {
                        if let Some(block) = self.parse_paragraph_or_heading(reader, doc)? {
                            cell_blocks.push(block);
                        }
                    }
                    b"w:tbl" => {
                        let inner_table = self.parse_table(reader, doc)?;
                        cell_blocks.push(Block::Table(inner_table));
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tc" => break,
                _ => (),
            }
            buf.clear();
        }

        Ok(TableCell {
            row_span,
            col_span,
            blocks: cell_blocks,
        })
    }

    // --- CAPTURE XML CHÍNH XÁC THEO ĐỘ SÂU (Cho OMML Formula) ---
    fn capture_xml(&self, reader: &mut Reader<&[u8]>, tag: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut xml = format!("<{}>", tag);
        let mut depth = 1;
        let mut buf = Vec::new();

        while depth > 0 {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    depth += 1;
                    xml.push_str(&String::from_utf8_lossy(&e.to_vec()));
                }
                Ok(Event::End(e)) => {
                    depth -= 1;
                    xml.push_str(&String::from_utf8_lossy(&e.to_vec()));
                }
                Ok(Event::Text(t)) => xml.push_str(&t.unescape()?),
                _ => (),
            }
            buf.clear();
        }
        Ok(xml)
    }
}
