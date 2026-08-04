use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct Styles {
    pub headings: HashMap<String, u8>,
}

impl Styles {
    pub fn parse(xml_content: &str) -> Self {
        let mut headings = HashMap::new();
        let mut reader = Reader::from_str(xml_content);
        let mut buf = Vec::new();
        let mut current_style_id = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:style" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"w:styleId" {
                            current_style_id = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"w:name" => {
                    if current_style_id.starts_with("Heading") {
                        if let Ok(level) = current_style_id.replace("Heading", "").parse::<u8>() {
                            headings.insert(current_style_id.clone(), level);
                        }
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"w:style" => {
                    current_style_id.clear();
                }
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }
        Styles { headings }
    }
}