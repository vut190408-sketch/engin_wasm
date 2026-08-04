use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct Relationships { pub map: HashMap<String, String> }
impl Relationships {
    pub fn parse(xml: &str) -> Self {
        let mut map = HashMap::new();
        let mut reader = Reader::from_str(xml);
        let mut buf = Vec::new();
        while let Ok(e) = reader.read_event_into(&mut buf) {
            if let Event::Start(ref s) = e {
                if s.name().as_ref() == b"Relationship" {
                    let mut id = ""; let mut target = "";
                    for attr in s.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" => id = std::str::from_utf8(&attr.value).unwrap_or(""),
                            b"Target" => target = std::str::from_utf8(&attr.value).unwrap_or(""),
                            _ => {}
                        }
                    }
                    map.insert(id.to_string(), target.to_string());
                }
            }
            buf.clear();
        }
        Relationships { map }
    }
}