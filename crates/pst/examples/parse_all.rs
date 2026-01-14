use clap::Parser;
use outlook_pst::{
    ltp::{
        prop_context::PropertyValue,
        table_context::{TableContext, TableRowData},
    },
    messaging::{
        folder::Folder as PstFolder,
        message::Message as PstMessage,
        store::Store,
    },
    ndb::node_id::NodeId,
};
use std::rc::Rc;

mod args;

mod parse_all_encoding {
    use compressed_rtf::*;
    use outlook_pst::ltp::prop_context::PropertyValue;

    pub fn decode_subject(value: &PropertyValue) -> Option<String> {
        match value {
            PropertyValue::String8(value) => {
                let offset = match value.buffer().first() {
                    Some(1) => 2,
                    _ => 0,
                };
                let buffer: Vec<_> = value
                    .buffer()
                    .iter()
                    .skip(offset)
                    .map(|&b| u16::from(b))
                    .collect();
                Some(String::from_utf16_lossy(&buffer))
            }
            PropertyValue::Unicode(value) => {
                let offset = match value.buffer().first() {
                    Some(1) => 2,
                    _ => 0,
                };
                Some(String::from_utf16_lossy(&value.buffer()[offset..]))
            }
            _ => None,
        }
    }

    pub fn decode_html_body(buffer: &[u8], code_page: u16) -> Option<String> {
        match code_page {
            20127 => {
                let buffer: Vec<_> = buffer.iter().map(|&b| u16::from(b)).collect();
                Some(String::from_utf16_lossy(&buffer))
            }
            _ => {
                let coding = codepage_strings::Coding::new(code_page).ok()?;
                Some(coding.decode(buffer).ok()?.to_string())
            }
        }
    }

    pub fn decode_rtf_compressed(buffer: &[u8]) -> Option<String> {
        decompress_rtf(buffer).ok()
    }
}

use parse_all_encoding as encoding;

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let store = outlook_pst::open_store(&args.file)?;

    println!("=== PST File Parser ===\n");
    println!("File: {}\n", args.file);

    // 1. ストアプロパティを出力
    print_store_properties(&store)?;

    // 2. 名前付きプロパティマップを出力
    print_named_properties(&store)?;

    // 3. フォルダ階層を再帰的に走査
    print_folder_hierarchy(&store)?;

    Ok(())
}

fn print_store_properties(store: &Rc<dyn Store>) -> anyhow::Result<()> {
    println!("=== Store Properties ===");
    let properties = store.properties();

    if let Ok(display_name) = properties.display_name() {
        println!("Display Name: {}", display_name);
    }

    if let Ok(ipm_sub_tree) = properties.ipm_sub_tree_entry_id() {
        println!("IPM Subtree Entry ID: {:?}", ipm_sub_tree);
    }

    if let Ok(deleted_items) = properties.ipm_wastebasket_entry_id() {
        println!("Deleted Items Entry ID: {:?}", deleted_items);
    }

    if let Ok(finder) = properties.finder_entry_id() {
        println!("Finder Entry ID: {:?}", finder);
    }

    println!("\nAll Store Properties:");
    for (prop_id, value) in properties.iter() {
        println!("  Property ID: 0x{prop_id:04X}, Value: {value:?}");
    }

    println!();
    Ok(())
}

fn print_named_properties(store: &Rc<dyn Store>) -> anyhow::Result<()> {
    println!("=== Named Properties ===");

    match store.named_property_map() {
        Ok(named_props) => {
            let properties = named_props.properties();

            match properties.stream_entry() {
                Ok(entries) => {
                    let mut count = 0;
                    for entry in entries {
                        count += 1;
                        let prop_id = entry.prop_id();
                        println!("Named Property ID: 0x{prop_id:04X}");

                        let guid = entry.guid();
                        match guid {
                            outlook_pst::messaging::named_prop::NamedPropertyGuid::None => {
                                println!("  GUID: None");
                            }
                            outlook_pst::messaging::named_prop::NamedPropertyGuid::Mapi => {
                                println!("  GUID: PS_MAPI");
                            }
                            outlook_pst::messaging::named_prop::NamedPropertyGuid::PublicStrings => {
                                println!("  GUID: PS_PUBLIC_STRINGS");
                            }
                            outlook_pst::messaging::named_prop::NamedPropertyGuid::GuidIndex(index) => {
                                if let Ok(guid) = properties.lookup_guid(
                                    outlook_pst::messaging::named_prop::NamedPropertyGuid::try_from(index)
                                        .unwrap_or(outlook_pst::messaging::named_prop::NamedPropertyGuid::None),
                                ) {
                                    println!("  GUID: {guid:?}");
                                }
                            }
                        }

                        match entry.id() {
                            outlook_pst::messaging::named_prop::NamedPropertyId::Number(id) => {
                                println!("  ID: Number(0x{id:08X})");
                            }
                            outlook_pst::messaging::named_prop::NamedPropertyId::StringOffset(index) => {
                                match properties.lookup_string(index) {
                                    Ok(string_entry) => {
                                        println!("  ID: String({:?})", string_entry.to_string());
                                    }
                                    Err(_) => {
                                        println!("  ID: StringOffset(0x{index:08X})");
                                    }
                                }
                            }
                        }
                    }
                    println!("Total named properties: {count}\n");
                }
                Err(e) => {
                    eprintln!("Warning: Failed to read named properties: {e:?}\n");
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to open named property map: {e:?}\n");
        }
    }

    Ok(())
}

fn print_folder_hierarchy(store: &Rc<dyn Store>) -> anyhow::Result<()> {
    println!("=== Folder Hierarchy ===");

    let ipm_sub_tree = match store.properties().ipm_sub_tree_entry_id() {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Error: Failed to get IPM Subtree Entry ID: {e:?}");
            return Ok(());
        }
    };

    let root_folder = match store.open_folder(&ipm_sub_tree) {
        Ok(folder) => folder,
        Err(e) => {
            eprintln!("Error: Failed to open IPM Subtree folder: {e:?}");
            return Ok(());
        }
    };

    let hierarchy_table = match root_folder.hierarchy_table() {
        Some(table) => table,
        None => {
            eprintln!("Warning: No hierarchy table found for IPM Subtree");
            return Ok(());
        }
    };

    let mut folders = Vec::new();

    for row in hierarchy_table.rows_matrix() {
        let node = NodeId::from(u32::from(row.id()));
        match store.properties().make_entry_id(node) {
            Ok(entry_id) => {
                match store.open_folder(&entry_id) {
                    Ok(folder) => folders.push(folder),
                    Err(e) => {
                        eprintln!("Warning: Failed to open folder (Node ID: 0x{:X}): {e:?}", u32::from(node));
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to make entry ID (Node ID: 0x{:X}): {e:?}", u32::from(node));
            }
        }
    }

    for folder in folders {
        print_folder(store, &folder, 0)?;
    }

    Ok(())
}

fn print_folder(store: &Rc<dyn Store>, folder: &Rc<dyn PstFolder>, indent: usize) -> anyhow::Result<()> {
    let indent_str = "  ".repeat(indent);
    let properties = folder.properties();

    let folder_name = match properties.display_name() {
        Ok(name) => name.to_string(),
        Err(_) => "(Unknown)".to_string(),
    };

    println!("{indent_str}📁 Folder: {folder_name}");

    // フォルダのプロパティを出力
    if indent == 0 {
        println!("{indent_str}  Properties:");
        for (prop_id, value) in properties.iter() {
            println!("{indent_str}    Property ID: 0x{prop_id:04X}, Value: {value:?}");
        }
    }

    // サブフォルダを再帰的に処理
    if let Some(hierarchy_table) = folder.hierarchy_table() {
        let mut sub_folders = Vec::new();

        for row in hierarchy_table.rows_matrix() {
            let node = NodeId::from(u32::from(row.id()));
            match folder.store().properties().make_entry_id(node) {
                Ok(entry_id) => {
                    match folder.store().open_folder(&entry_id) {
                        Ok(sub_folder) => sub_folders.push(sub_folder),
                        Err(e) => {
                            eprintln!("{indent_str}  Warning: Failed to open sub-folder: {e:?}");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{indent_str}  Warning: Failed to make entry ID for sub-folder: {e:?}");
                }
            }
        }

        for sub_folder in sub_folders {
            print_folder(store, &sub_folder, indent + 1)?;
        }
    }

    // メッセージを処理
    if let Some(contents_table) = folder.contents_table() {
        let mut message_count = 0;

        for row in contents_table.rows_matrix() {
            message_count += 1;
            match process_message(store, contents_table.as_ref(), row, indent + 1) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{indent_str}  Warning: Failed to process message: {e:?}");
                }
            }
        }

        if message_count > 0 {
            println!("{indent_str}  Total messages in folder: {message_count}");
        }
    }

    println!();
    Ok(())
}

fn process_message(
    store: &Rc<dyn Store>,
    _table: &dyn TableContext,
    row: &TableRowData,
    indent: usize,
) -> anyhow::Result<()> {
    let indent_str = "  ".repeat(indent);
    let node = NodeId::from(u32::from(row.id()));
    let entry_id = store.properties().make_entry_id(node)?;

    let message = match store.open_message(&entry_id, None) {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("{indent_str}Warning: Failed to open message: {e:?}");
            return Ok(());
        }
    };

    let properties = message.properties();

    // 件名
    let subject = properties
        .get(0x0037)
        .and_then(encoding::decode_subject)
        .unwrap_or_else(|| "(no subject)".to_string());

    println!("{indent_str}📧 Message: {subject}");

    // メッセージクラス
    if let Ok(message_class) = properties.message_class() {
        println!("{indent_str}  Message Class: {message_class}");
    }

    // 送信者
    if let Some(sender) = properties.get(0x0C1A) {
        match sender {
            PropertyValue::String8(value) => println!("{indent_str}  From: {}", value.to_string()),
            PropertyValue::Unicode(value) => println!("{indent_str}  From: {}", value.to_string()),
            _ => {}
        }
    }

    // 受信日時
    if let Some(received_time) = properties.get(0x0E06) {
        if let PropertyValue::Time(time) = received_time {
            println!("{indent_str}  Received: {}", format_time(*time));
        }
    }

    // 作成日時
    if let Ok(creation_time) = properties.creation_time() {
        println!("{indent_str}  Created: {}", format_time(creation_time));
    }

    // 受信者を処理
    print_recipients(&message, indent + 1)?;

    // 添付ファイルを処理
    print_attachments(&message, store, indent + 1)?;

    // 本文を処理
    print_message_body(&message, indent + 1)?;

    // その他のプロパティ
    println!("{indent_str}  Other Properties:");
    for (prop_id, value) in properties.iter() {
        // 既に表示したプロパティはスキップ
        if matches!(prop_id, 0x0037 | 0x0C1A | 0x0E06 | 0x3007 | 0x001A) {
            continue;
        }
        println!("{indent_str}    Property ID: 0x{prop_id:04X}, Value: {value:?}");
    }

    println!();
    Ok(())
}

fn print_recipients(message: &Rc<dyn PstMessage>, indent: usize) -> anyhow::Result<()> {
    let indent_str = "  ".repeat(indent);
    let recipient_table = message.recipient_table();
    let context = recipient_table.context();

    let mut recipients = Vec::new();

    for row in recipient_table.rows_matrix() {
        let columns: Vec<_> = match row.columns(context) {
            Ok(cols) => cols,
            Err(_) => continue,
        };

        let mut recipient_type = None;
        let mut display_name = None;

        for (col, value) in context.columns().iter().zip(columns.iter()) {
            if col.prop_id() == 0x0C15 {
                // Recipient Type
                if let Some(value) = value.as_ref() {
                    if let Ok(PropertyValue::Integer32(rt)) = recipient_table.read_column(value, col.prop_type()) {
                        recipient_type = Some(rt);
                    }
                }
            } else if col.prop_id() == 0x3001 {
                // Display Name
                if let Some(value) = value.as_ref() {
                    match recipient_table.read_column(value, col.prop_type()) {
                        Ok(PropertyValue::String8(name)) => {
                            display_name = Some(name.to_string());
                        }
                        Ok(PropertyValue::Unicode(name)) => {
                            display_name = Some(name.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        if let (Some(rt), Some(name)) = (recipient_type, display_name) {
            recipients.push((rt, name));
        }
    }

    if !recipients.is_empty() {
        println!("{indent_str}Recipients:");
        for (rt, name) in recipients {
            let rt_str = match rt {
                1 => "To",
                2 => "CC",
                3 => "BCC",
                _ => "Unknown",
            };
            println!("{indent_str}  {rt_str}: {name}");
        }
    }

    Ok(())
}

fn print_attachments(
    message: &Rc<dyn PstMessage>,
    _store: &Rc<dyn Store>,
    indent: usize,
) -> anyhow::Result<()> {
    let indent_str = "  ".repeat(indent);

    let attachment_table = match message.attachment_table() {
        Some(table) => table,
        None => return Ok(()),
    };

    let context = attachment_table.context();
    let mut attachment_count = 0;

    for row in attachment_table.rows_matrix() {
        attachment_count += 1;

        // 添付ファイルのプロパティをテーブルから読み取る
        let mut filename = None;
        let mut size = None;
        let mut method = None;

        let columns: Vec<_> = match row.columns(context) {
            Ok(cols) => cols,
            Err(e) => {
                eprintln!("{indent_str}Warning: Failed to read attachment columns: {e:?}");
                continue;
            }
        };

        for (col, value) in context.columns().iter().zip(columns.iter()) {
            if let Some(value) = value.as_ref() {
                match attachment_table.read_column(value, col.prop_type()) {
                    Ok(PropertyValue::String8(s)) if col.prop_id() == 0x3704 => {
                        filename = Some(s.to_string());
                    }
                    Ok(PropertyValue::Unicode(s)) if col.prop_id() == 0x3704 => {
                        filename = Some(s.to_string());
                    }
                    Ok(PropertyValue::Integer32(s)) if col.prop_id() == 0x0E20 => {
                        size = Some(s);
                    }
                    Ok(PropertyValue::Integer32(m)) if col.prop_id() == 0x3705 => {
                        method = Some(m);
                    }
                    _ => {}
                }
            }
        }

        let filename = filename.unwrap_or_else(|| format!("attachment_{attachment_count}"));
        println!("{indent_str}📎 Attachment {attachment_count}: {filename}");

        if let Some(s) = size {
            println!("{indent_str}  Size: {} bytes", s);
        }

        if let Some(m) = method {
            match m {
                0 => println!("{indent_str}  Method: None"),
                1 => println!("{indent_str}  Method: ByValue"),
                2 => println!("{indent_str}  Method: ByReference"),
                4 => println!("{indent_str}  Method: ByReferenceOnly"),
                5 => println!("{indent_str}  Method: EmbeddedMessage"),
                6 => println!("{indent_str}  Method: Storage"),
                _ => println!("{indent_str}  Method: Unknown({})", m),
            }
        }
    }

    if attachment_count > 0 {
        println!("{indent_str}Total attachments: {attachment_count}");
    }

    Ok(())
}

fn print_message_body(message: &Rc<dyn PstMessage>, indent: usize) -> anyhow::Result<()> {
    let indent_str = "  ".repeat(indent);
    let properties = message.properties();

    // テキスト本文 (0x1000)
    if let Some(body) = properties.get(0x1000) {
        match body {
            PropertyValue::String8(value) => {
                let text = value.to_string();
                if !text.is_empty() {
                    println!("{indent_str}Body (Text):");
                    print_text_preview(&text, indent + 1);
                }
            }
            PropertyValue::Unicode(value) => {
                let text = value.to_string();
                if !text.is_empty() {
                    println!("{indent_str}Body (Text):");
                    print_text_preview(&text, indent + 1);
                }
            }
            _ => {}
        }
    }

    // HTML本文 (0x1013)
    if let Some(html) = properties.get(0x1013) {
        match html {
            PropertyValue::Binary(value) => {
                let code_page = properties
                    .get(0x3FDE)
                    .and_then(|v| match v {
                        PropertyValue::Integer32(cp) => u16::try_from(*cp).ok(),
                        _ => None,
                    })
                    .unwrap_or(1252);

                if let Some(html_text) = encoding::decode_html_body(value.buffer(), code_page) {
                    if !html_text.is_empty() {
                        println!("{indent_str}Body (HTML):");
                        print_text_preview(&html_text, indent + 1);
                    }
                }
            }
            PropertyValue::String8(value) => {
                let text = value.to_string();
                if !text.is_empty() {
                    println!("{indent_str}Body (HTML):");
                    print_text_preview(&text, indent + 1);
                }
            }
            PropertyValue::Unicode(value) => {
                let text = value.to_string();
                if !text.is_empty() {
                    println!("{indent_str}Body (HTML):");
                    print_text_preview(&text, indent + 1);
                }
            }
            _ => {}
        }
    }

    // RTF本文 (0x1009)
    if let Some(rtf) = properties.get(0x1009) {
        if let PropertyValue::Binary(value) = rtf {
            if let Some(rtf_text) = encoding::decode_rtf_compressed(value.buffer()) {
                if !rtf_text.is_empty() {
                    println!("{indent_str}Body (RTF):");
                    print_text_preview(&rtf_text, indent + 1);
                }
            }
        }
    }

    Ok(())
}

fn print_text_preview(text: &str, indent: usize) {
    let indent_str = "  ".repeat(indent);
    let preview = if text.len() > 200 {
        format!("{}...", &text[..200])
    } else {
        text.to_string()
    };

    for line in preview.lines().take(5) {
        println!("{indent_str}{}", line);
    }

    if text.lines().count() > 5 || text.len() > 200 {
        println!("{indent_str}... (truncated)");
    }
}

fn format_time(time: i64) -> String {
    // Windows FILETIME to Unix timestamp conversion
    // FILETIME is 100-nanosecond intervals since January 1, 1601
    // Unix timestamp is seconds since January 1, 1970
    let unix_epoch = 116444736000000000i64; // January 1, 1970 in FILETIME
    let seconds = (time - unix_epoch) / 10_000_000;

    // Simple date formatting without external dependencies
    // Convert seconds to a readable date
    let days_since_epoch = seconds / 86400;
    let secs_in_day = seconds % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let secs = secs_in_day % 60;

    // Simple year calculation (approximate)
    let year = 1970 + (days_since_epoch / 365);
    let day_of_year = days_since_epoch % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{secs:02} (FILETIME: {time})")
}
