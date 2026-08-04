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
            if let Event::Start(s) = e {
                if s.name().as_ref() == b"Relationship" {
                    let mut id = String::new(); let mut target = String::new();
                    for attr in s.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" => id = String::from_utf8_lossy(&attr.value).into_owned(),
                            b"Target" => target = String::from_utf8_lossy(&attr.value).into_owned(),
                            _ => {}
                        }
                    }
                    map.insert(id, target);
                }
            }
            buf.clear();
        }
        Relationships { map }
    }
}
