use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct Numbering {
    // Lưu map numId -> abstractNumId
    pub num_map: HashMap<String, String>,
}

impl Numbering {
    pub fn parse(xml_content: &str) -> Self {
        let mut num_map = HashMap::new();
        let mut reader = Reader::from_str(xml_content);
        let mut buf = Vec::new();
        let mut current_num_id = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:num" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"w:numId" {
                            current_num_id = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:abstractNumId" => {
                    if let Ok(val) = e.try_get_attribute(b"w:val") {
                        if let Some(attr) = val {
                            let abstract_id = String::from_utf8_lossy(&attr.value).to_string();
                            num_map.insert(current_num_id.clone(), abstract_id);
                        }
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:num" => {
                    current_num_id.clear();
                }
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }
        Numbering { num_map }
    }
}