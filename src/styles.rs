use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct Styles {
    // Lưu map: style_id (ví dụ "Heading1" hoặc "a0") -> cấp độ Heading (1..=9)
    pub heading_levels: HashMap<String, u8>,
}

impl Styles {
    pub fn parse(xml_content: &str) -> Self {
        let mut heading_levels = HashMap::new();
        let mut reader = Reader::from_str(xml_content);
        let mut buf = Vec::new();

        let mut current_style_id = String::new();
        let mut current_outline_lvl: Option<u8> = None;
        let mut is_heading_by_name = false;

        while let Ok(e) = reader.read_event_into(&mut buf) {
            match e {
                Event::Start(ref s) if s.name().as_ref() == b"w:style" => {
                    current_style_id.clear();
                    current_outline_lvl = None;
                    is_heading_by_name = false;

                    for attr in s.attributes().flatten() {
                        if attr.key.as_ref() == b"w:styleId" {
                            current_style_id = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
                Event::Start(ref s) | Event::Empty(ref s) if s.name().as_ref() == b"w:name" => {
                    for attr in s.attributes().flatten() {
                        if attr.key.as_ref() == b"w:val" {
                            let val = String::from_utf8_lossy(&attr.value);
                            if val.starts_with("heading ") || val.starts_with("Heading ") {
                                is_heading_by_name = true;
                                if let Ok(lvl) = val.replace("heading ", "").replace("Heading ", "").trim().parse::<u8>() {
                                    current_outline_lvl = Some(lvl);
                                }
                            }
                        }
                    }
                }
                Event::Start(ref s) | Event::Empty(ref s) if s.name().as_ref() == b"w:outlineLvl" => {
                    for attr in s.attributes().flatten() {
                        if attr.key.as_ref() == b"w:val" {
                            if let Ok(lvl) = String::from_utf8_lossy(&attr.value).parse::<u8>() {
                                // Trong XML của Word, outlineLvl bắt đầu từ 0 (0 = Heading 1)
                                current_outline_lvl = Some(lvl + 1);
                            }
                        }
                    }
                }
                Event::End(ref s) if s.name().as_ref() == b"w:style" => {
                    if !current_style_id.is_empty() {
                        if let Some(lvl) = current_outline_lvl {
                            heading_levels.insert(current_style_id.clone(), lvl);
                        } else if is_heading_by_name || current_style_id.starts_with("Heading") {
                            if let Ok(lvl) = current_style_id.replace("Heading", "").parse::<u8>() {
                                heading_levels.insert(current_style_id.clone(), lvl);
                            }
                        }
                    }
                }
                _ => {}
            }
            buf.clear();
        }

        Styles { heading_levels }
    }

    pub fn get_heading_level(&self, style_id: &str) -> Option<u8> {
        self.heading_levels.get(style_id).copied()
    }
}
