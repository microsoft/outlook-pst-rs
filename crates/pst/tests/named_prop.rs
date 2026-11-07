use std::collections::BTreeSet;

use outlook_pst::{messaging::named_prop::NamedPropertyId, open_store};

#[test]
fn test_stream_string_reads_every_name() {
    // Every name the entry stream of Empty.pst points at should be in stream_string().
    let store = open_store(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/Empty.pst")).unwrap();
    let map = store.named_property_map().unwrap();
    let properties = map.properties();
    let offsets: BTreeSet<u32> = properties
        .stream_entry()
        .unwrap()
        .iter()
        .filter_map(|entry| match entry.id() {
            NamedPropertyId::StringOffset(offset) => Some(offset),
            NamedPropertyId::Number(_) => None,
        })
        .collect();
    let expected: Vec<String> = offsets
        .iter()
        .map(|offset| properties.lookup_string(*offset).unwrap().to_string())
        .collect();
    let names: Vec<String> = properties
        .stream_string()
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(names, expected);
}
