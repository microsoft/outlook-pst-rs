# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/microsoft/outlook-pst-rs/releases/tag/v0.1.0) - 2025-03-08

### Added

- split browse_pst into ANSI and Unicode versions
- add a body preview pane to browse_pst
- initial interative browse_pst sample
- allow filtereing property IDs when loading messages
- implement search update (basic) queue types
- implement search update types in messaging layer
- implement named property types in messaging layer
- implement attachment types in messaging layer
- load recipient and attachment tables on messages
- implement message types in messaging layer
- implement folder types in messaging layer
- required store props and entry ID conversion
- implement store types in messaging layer
- implement read/write table context types
- implement read/write property context types
- lookup by key in node/block BTree pages
- implement read/write tree-on-heap structures
- implement read/write heap-on-node structures
- finish implementing read/write blocks
- implement read/write Data Tree blocks
- implement read/write data blocks
- read/write BTree page dynamically with level
- generic BTree page read/write support
- implement read/write BTree intermediate pages
- implement read/write Density List page
- implement read/write page trailers and map pages
- implement read/write NDB header support
- start work on the NDB layer

### Fixed

- cleanup commented out code
- simplify taking ownership of data blocks
- bounds check heap block index for 1..8 range
- ignore per-block padding in row matrix
- handle empty HeapId values in property context
- cleanup error message
- *(doc)* attachment module comment
- cleanup and deduplication in store module
- *(doc)* message module comment
- empty table sets row matrix HNID to 0
- empty tree-on-heap sets root HID to 0
- tweak the output of read_root_folder
- test the table context
- read property context sub-nodes from sub-node tree
- misc clippy fixes
- test the property context and tree-on-heap
- add accessors instead of making members public
- make HeapNodePageAlloc members public
- test the data-tree block traversal
- test the sub-node tree traversal
- handle loosely packed BTree entries
- data validation in heap and tree modules
- hide read/write support traits in pub(crate) module
- move ENTRY_SIZE into private DataTreeBlockExt trait
- misc. cleanup and check for internal BIDs in data blocks
- test BTrees against additional sample PSTs
- test Node and Block BTrees
- cleanup dwPadding in AnsiBTreePage
- test Density List and add CRC verification/update to page read/write
- test and fix NDB header parsing with an empty Unicode PST
- grammar in README

### Other

- scaffold the LTP layer module
- expose ndb sub-module hierarchy
- refactor ndb module into sub-modules
- cargo fmt
- cargo fmt
