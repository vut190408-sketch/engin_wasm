use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use crate::models::NumberingMetadata;

#[derive(Default)]
struct AbstractNum {
    // ilvl (cấp độ 0, 1, 2...) -> numFmt ("decimal", "bullet", "lowerLetter", "upperRoman"...)
    levels: HashMap<u32, String>,
}

pub struct Numbering {
    // Map: numId -> abstractNumId
    num_to_abstract: HashMap<String, String>,
    // Map: abstractNumId -> AbstractNum
    abstract_nums: HashMap<String, AbstractNum>,
}

impl Numbering {
    pub fn parse(xml_content: &str) -> Self {
        let mut num_to_abstract = HashMap::new();
        let mut abstract_nums = HashMap::new();
        let mut reader = Reader::from_str(xml_content);
        let mut buf = Vec::new();

        let mut current_num_id = String::new();
        let mut current_abstract_id = String::new();
        let mut current_abstract_num = AbstractNum::default();
        let mut current_ilvl: Option<u32> = None;

        while let Ok(e) = reader.read_event_into(&mut buf) {
            match e {
                // Đọc định nghĩa abstractNum
                Event::Start(ref s) if s.name().as_ref() == b"w:abstractNum" => {
                    current_abstract_id.clear();
                    current_abstract_num = AbstractNum::default();
                    for attr in s.attributes().flatten() {
                        if attr.key.as_ref() == b"w:abstractNumId" {
                            current_abstract_id = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
                Event::Start(ref s) if s.name().as_ref() == b"w:lvl" => {
                    current_ilvl = None;
                    for attr in s.attributes().flatten() {
                        if attr.key.as_ref() == b"w:ilvl" {
                            if let Ok(lvl) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                current_ilvl = Some(lvl);
                            }
                        }
                    }
                }
                Event::Start(ref s) | Event::Empty(ref s) if s.name().as_ref() == b"w:numFmt" => {
                    if let Some(ilvl) = current_ilvl {
                        for attr in s.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let fmt = String::from_utf8_lossy(&attr.value).to_string();
                                current_abstract_num.levels.insert(ilvl, fmt);
                            }
                        }
                    }
                }
                Event::End(ref s) if s.name().as_ref() == b"w:abstractNum" => {
                    if !current_abstract_id.is_empty() {
                        abstract_nums.insert(current_abstract_id.clone(), current_abstract_num);
                        current_abstract_num = AbstractNum::default();
                    }
                }

                // Đọc ánh xạ numId -> abstractNumId
                Event::Start(ref s) if s.name().as_ref() == b"w:num" => {
                    current_num_id.clear();
                    for attr in s.attributes().flatten() {
                        if attr.key.as_ref() == b"w:numId" {
                            current_num_id = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
                Event::Start(ref s) | Event::Empty(ref s) if s.name().as_ref() == b"w:abstractNumId" => {
                    if !current_num_id.is_empty() {
                        for attr in s.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let abs_id = String::from_utf8_lossy(&attr.value).to_string();
                                num_to_abstract.insert(current_num_id.clone(), abs_id);
                            }
                        }
                    }
                }

                _ => {}
            }
            buf.clear();
        }

        Numbering {
            num_to_abstract,
            abstract_nums,
        }
    }

    pub fn get_metadata(&self, num_id: &str, ilvl: u32) -> Option<NumberingMetadata> {
        let abs_id = self.num_to_abstract.get(num_id)?;
        let abs_num = self.abstract_nums.get(abs_id)?;
        let kind = abs_num.levels.get(&ilvl)?.clone();

        Some(NumberingMetadata {
            kind,
            level: ilvl,
        })
    }
}
