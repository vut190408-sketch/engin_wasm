use crate::models::*;
use crate::relationships::Relationships;
use crate::styles::Styles;
use crate::numbering::Numbering;
use std::io::Read;
use zip::ZipArchive;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Cursor;

pub struct DocxParser {
    archive: ZipArchive<Cursor<Vec<u8>>>,
    pub rels: Relationships,
    pub styles: Styles,
    pub numbering: Numbering,
}

impl DocxParser {
    pub fn new(file_bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let reader = Cursor::new(file_bytes.to_vec());
        let mut archive = ZipArchive::new(reader)?;

        let read_xml = |name: &str| -> String {
            let mut s = String::new();
            if let Ok(mut f) = archive.by_name(name) { let _ = f.read_to_string(&mut s); }
            s
        };

        Ok(Self {
            archive,
            rels: Relationships::parse(&read_xml("word/_rels/document.xml.rels")),
            styles: Styles::parse(&read_xml("word/styles.xml")),
            numbering: Numbering::parse(&read_xml("word/numbering.xml")),
        })
    }

    pub fn parse(&mut self) -> Result<Document, Box<dyn std::error::Error>> {
        let mut doc = Document { version: 1, blocks: Vec::new(), images: Default::default(), formulas: Default::default() };
        let xml = self.read_file("word/document.xml")?;
        let mut reader = Reader::from_str(&xml);
        reader.trim_text(false);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"w:p" => doc.blocks.push(Block::Paragraph(self.parse_p(&mut reader, &mut doc)?)),
                    b"w:tbl" => doc.blocks.push(Block::Table(self.parse_tbl(&mut reader, &mut doc)?)),
                    _ => (),
                },
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(doc)
    }

    fn parse_p(&self, reader: &mut Reader<&[u8]>, doc: &mut Document) -> Result<Paragraph, Box<dyn std::error::Error>> {
        let mut p = Paragraph { id: format!("p_{}", uuid::Uuid::new_v4()), text: String::new(), ..Default::default() };
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:r" => self.parse_run(reader, &mut p, doc)?,
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:p" => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(p)
    }

    fn parse_tbl(&self, reader: &mut Reader<&[u8]>, doc: &mut Document) -> Result<Table, Box<dyn std::error::Error>> {
        let mut table = Table { id: format!("tbl_{}", uuid::Uuid::new_v4()), rows: Vec::new() };
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:tr" => {
                    let mut row = Vec::new();
                    // Parse w:tc (cell) ...
                    table.rows.push(row);
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:tbl" => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(table)
    }

    fn parse_run(&self, reader: &mut Reader<&[u8]>, p: &mut Paragraph, doc: &mut Document) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"w:t" => if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) { p.text.push_str(&t.unescape()?); },
                    b"m:oMath" => {
                        let xml = self.capture_xml(reader, "m:oMath")?;
                        let id = format!("f_{}", uuid::Uuid::new_v4());
                        doc.formulas.insert(id.clone(), xml);
                        p.inline_objects.push(InlineObject { position: p.text.chars().count(), id, kind: "formula".into(), ratio: None });
                    }
                    _ => (),
                },
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:r" => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(())
    }

    fn capture_xml(&self, reader: &mut Reader<&[u8]>, tag: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut xml = format!("<{}>", tag);
        let mut depth = 1;
        let mut buf = Vec::new();
        while depth > 0 {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => { depth += 1; xml.push_str(&String::from_utf8_lossy(&e.to_vec())); }
                Ok(Event::End(e)) => { depth -= 1; xml.push_str(&String::from_utf8_lossy(&e.to_vec())); }
                Ok(Event::Text(t)) => xml.push_str(&t.unescape()?),
                _ => (),
            }
            buf.clear();
        }
        Ok(xml)
    }

    fn read_file(&mut self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut s = String::new();
        self.archive.by_name(name)?.read_to_string(&mut s)?;
        Ok(s)
    }
}