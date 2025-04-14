# outlook-pst

The PST file format is publicly documented in the [MS-PST](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/141923d5-15ab-4ef1-a524-6dce75aae546) open specification. Data structures and type names generally mimic the concepts and names in that document, with some adjustment for readability and to match Rust language conventions. As much as possible, everything in this crate should have a deep link to the documentation it is based on in the doc comments. 

## Unimplemented: PST file modification

This project is suitable for read-only access to PST files, but as with previous public implementations of the PST format, we've decided to avoid complicating it with full write support.

However, this version does support [Crash Recovery and AMap Rebuilding](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/d9bcc1fd-c66a-41b3-b6d7-ed09d2a25ced), which is a step towards supporting [Transactional Semantics](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/bc5a92df-7fc1-4dc2-9c7c-5677237dd73a) when modifying a PST file. If you plan on implementing PST file modification, you can use this as a reference for those features.

If you choose to modify the PST files, please be careful to follow all of the guidance in the [Maintaining Data Integrity](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/5e1a4d6b-ebbf-4658-9aa7-824929233044) section of the specification to avoid corrupting your PST files in a way that prevents Outlook (or this library) from opening them anymore.
