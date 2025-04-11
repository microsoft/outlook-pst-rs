#![allow(dead_code)]

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Style, Stylize},
    symbols::{self, border},
    text::Line,
    widgets::{Block, Borders, List, ListState, Paragraph, StatefulWidget, Widget},
    DefaultTerminal, Frame,
};
use std::{cell::OnceCell, io, sync::Arc};

use outlook_pst::{
    ltp::{
        prop_context::PropertyValue,
        table_context::{TableRowData, UnicodeTableContext},
    },
    messaging::{
        attachment::UnicodeAttachment,
        folder::UnicodeFolder,
        message::UnicodeMessage,
        store::{EntryId, UnicodeStore},
    },
    ndb::node_id::NodeId,
    *,
};

mod args;
mod encoding;

struct IpmSubTree {
    display_name: OnceCell<anyhow::Result<String>>,
    root_folders: OnceCell<anyhow::Result<Vec<Folder>>>,
    pst_file: Arc<UnicodePstFile>,
    pst_store: OnceCell<anyhow::Result<Arc<UnicodeStore>>>,
    pst_folders: OnceCell<anyhow::Result<Vec<Arc<UnicodeFolder>>>>,
}

impl IpmSubTree {
    fn new(pst: UnicodePstFile) -> Self {
        Self {
            display_name: Default::default(),
            root_folders: Default::default(),
            pst_file: Arc::new(pst),
            pst_store: Default::default(),
            pst_folders: Default::default(),
        }
    }

    fn display_name(&self) -> String {
        self.display_name
            .get_or_init(|| Ok(self.store()?.properties().display_name()?))
            .as_ref()
            .map_err(|err| anyhow::anyhow!("{err:?}"))
            .map(String::as_str)
            .unwrap_or_else(|_| "(Missing Store Name)")
            .to_string()
    }

    fn store(&self) -> anyhow::Result<Arc<UnicodeStore>> {
        self.pst_store
            .get_or_init(|| Ok(UnicodeStore::read(self.pst_file.clone())?))
            .as_ref()
            .map(Clone::clone)
            .map_err(|err| anyhow::anyhow!("{err:?}"))
    }

    fn root_folders(&self) -> anyhow::Result<&[Folder]> {
        Ok(self
            .root_folders
            .get_or_init(|| {
                let mut root_folders = self
                    .pst_folders
                    .get_or_init(|| {
                        let ipm_sub_tree = self.store()?.properties().ipm_sub_tree_entry_id()?;
                        let ipm_subtree_folder = UnicodeFolder::read(self.store()?, &ipm_sub_tree)?;
                        let hierarchy_table = ipm_subtree_folder.hierarchy_table().ok_or(
                            anyhow::anyhow!("No hierarchy table found for the IPM Subtree."),
                        )?;

                        hierarchy_table
                            .rows_matrix()
                            .map(|row| {
                                let node = NodeId::from(u32::from(row.id()));
                                let entry_id = self.store()?.properties().make_entry_id(node)?;
                                Ok(UnicodeFolder::read(self.store()?, &entry_id)?)
                            })
                            .collect()
                    })
                    .as_ref()
                    .map_err(|err| anyhow::anyhow!("{err:?}"))?
                    .iter()
                    .map(Clone::clone)
                    .map(Folder::new)
                    .collect::<Result<Vec<_>, _>>()?;
                root_folders.sort_by(|a, b| a.name.cmp(&b.name));

                Ok(root_folders)
            })
            .as_ref()
            .map_err(|err| anyhow::anyhow!("{err:?}"))?
            .as_slice())
    }
}

struct Folder {
    name: String,
    sub_folders: OnceCell<anyhow::Result<Vec<Folder>>>,
    messages: OnceCell<anyhow::Result<Vec<Message>>>,
    pst_folder: Arc<UnicodeFolder>,
    pst_sub_folders: OnceCell<anyhow::Result<Vec<Arc<UnicodeFolder>>>>,
}

impl Folder {
    fn new(folder: Arc<UnicodeFolder>) -> anyhow::Result<Self> {
        let properties = folder.properties();
        let name = properties.display_name()?.to_string();

        Ok(Self {
            name,
            sub_folders: Default::default(),
            messages: Default::default(),
            pst_folder: folder,
            pst_sub_folders: Default::default(),
        })
    }

    fn sub_folders(&self) -> anyhow::Result<&[Folder]> {
        Ok(self
            .sub_folders
            .get_or_init(|| {
                let mut sub_folders =
                    self.pst_sub_folders
                        .get_or_init(|| {
                            let hierarchy_table = self.pst_folder.hierarchy_table().ok_or(
                                anyhow::anyhow!("No hierarchy table found for the folder."),
                            )?;

                            hierarchy_table
                                .rows_matrix()
                                .map(|row| {
                                    let node = NodeId::from(u32::from(row.id()));
                                    let entry_id =
                                        self.pst_folder.store().properties().make_entry_id(node)?;
                                    Ok(UnicodeFolder::read(
                                        self.pst_folder.store().clone(),
                                        &entry_id,
                                    )?)
                                })
                                .collect()
                        })
                        .as_ref()
                        .map_err(|err| anyhow::anyhow!("{err:?}"))?
                        .iter()
                        .map(Clone::clone)
                        .map(Folder::new)
                        .collect::<Result<Vec<_>, _>>()?;
                sub_folders.sort_by(|a, b| a.name.cmp(&b.name));

                Ok(sub_folders)
            })
            .as_ref()
            .map_err(|err| anyhow::anyhow!("{err:?}"))?
            .as_slice())
    }

    fn messages(&self) -> anyhow::Result<&[Message]> {
        self.messages
            .get_or_init(|| {
                let contents_table = self
                    .pst_folder
                    .contents_table()
                    .ok_or(anyhow::anyhow!("No contents table found for the folder."))?;
                contents_table
                    .rows_matrix()
                    .map(|row| Message::new(self.pst_folder.store().clone(), contents_table, row))
                    .collect::<anyhow::Result<Vec<_>>>()
            })
            .as_ref()
            .map(Vec::as_slice)
            .map_err(|err| anyhow::anyhow!("{err:?}"))
    }
}

enum MessageOrRow {
    Message(Arc<UnicodeMessage>),
    Row {
        subject: Option<String>,
        received_time: i64,
    },
}

struct Message {
    entry_id: EntryId,
    message: MessageOrRow,
    recipients: OnceCell<Vec<Recipient>>,
    body: OnceCell<anyhow::Result<Option<Body>>>,
    attachments: OnceCell<anyhow::Result<Vec<Attachment>>>,
    pst_store: Arc<UnicodeStore>,
    pst_message: OnceCell<anyhow::Result<Arc<UnicodeMessage>>>,
    pst_full_message: OnceCell<anyhow::Result<Arc<UnicodeMessage>>>,
    pst_attachments: OnceCell<anyhow::Result<Vec<Arc<UnicodeAttachment>>>>,
}

impl Message {
    fn new(
        store: Arc<UnicodeStore>,
        table: &UnicodeTableContext,
        row: &TableRowData,
    ) -> anyhow::Result<Self> {
        let entry_id = store
            .properties()
            .make_entry_id(NodeId::from(u32::from(row.id())))?;

        let context = table.context();
        let subject_col = context
            .columns()
            .iter()
            .position(|col| col.prop_id() == 0x0037);
        let received_col = context
            .columns()
            .iter()
            .position(|col| col.prop_id() == 0x0E06);

        Ok(match (subject_col, received_col) {
            (Some(subject_col), Some(received_col)) => {
                let columns = row.columns(context)?;
                let subject = columns[subject_col]
                    .as_ref()
                    .and_then(|value| {
                        store
                            .read_table_column(
                                table,
                                value,
                                context.columns()[subject_col].prop_type(),
                            )
                            .ok()
                    })
                    .as_ref()
                    .and_then(encoding::decode_subject);

                let received_time = columns[received_col]
                    .as_ref()
                    .and_then(|value| {
                        store
                            .read_table_column(
                                table,
                                value,
                                context.columns()[received_col].prop_type(),
                            )
                            .ok()
                    })
                    .and_then(|value| match value {
                        PropertyValue::Time(value) => Some(value),
                        _ => None,
                    })
                    .unwrap_or(0);

                Self {
                    entry_id,
                    message: MessageOrRow::Row {
                        subject,
                        received_time,
                    },
                    recipients: Default::default(),
                    body: Default::default(),
                    attachments: Default::default(),
                    pst_store: store,
                    pst_message: Default::default(),
                    pst_full_message: Default::default(),
                    pst_attachments: Default::default(),
                }
            }
            _ => {
                let message = MessageOrRow::Message(UnicodeMessage::read(
                    store.clone(),
                    &entry_id,
                    Some(&[0x0037, 0x0E06]),
                )?);

                Self {
                    entry_id,
                    message,
                    recipients: Default::default(),
                    body: Default::default(),
                    attachments: Default::default(),
                    pst_store: store,
                    pst_message: Default::default(),
                    pst_full_message: Default::default(),
                    pst_attachments: Default::default(),
                }
            }
        })
    }

    fn message(&self) -> anyhow::Result<Arc<UnicodeMessage>> {
        match &self.message {
            MessageOrRow::Message(message) => Ok(message.clone()),
            MessageOrRow::Row { .. } => self
                .pst_message
                .get_or_init(|| {
                    Ok(UnicodeMessage::read(
                        self.pst_store.clone(),
                        &self.entry_id,
                        Some(&[0x0037, 0x0E06]),
                    )?)
                })
                .as_ref()
                .map(Clone::clone)
                .map_err(|err| anyhow::anyhow!("{err:?}")),
        }
    }

    fn full_message(&self) -> anyhow::Result<Arc<UnicodeMessage>> {
        self.pst_full_message
            .get_or_init(|| {
                Ok(UnicodeMessage::read(
                    self.pst_store.clone(),
                    &self.entry_id,
                    None,
                )?)
            })
            .as_ref()
            .map(Clone::clone)
            .map_err(|err| anyhow::anyhow!("{err:?}"))
    }

    fn subject(&self) -> anyhow::Result<Option<String>> {
        match &self.message {
            MessageOrRow::Message(message) => {
                let properties = message.properties();
                Ok(properties.get(0x0037).and_then(encoding::decode_subject))
            }
            MessageOrRow::Row { subject, .. } => Ok(subject.clone()),
        }
    }

    fn received_time(&self) -> anyhow::Result<i64> {
        match &self.message {
            MessageOrRow::Message(message) => {
                let properties = message.properties();
                match properties.get(0x0E06) {
                    Some(PropertyValue::Time(value)) => Ok(*value),
                    _ => Err(anyhow::anyhow!("Received time not found")),
                }
            }
            MessageOrRow::Row { received_time, .. } => Ok(*received_time),
        }
    }

    fn recipients(&self) -> anyhow::Result<&[Recipient]> {
        Ok(self
            .recipients
            .get_or_init(|| {
                let Ok(message) = self.message() else {
                    return Default::default();
                };

                let recipient_table = message.recipient_table();
                let context = recipient_table.context();

                recipient_table
                    .rows_matrix()
                    .filter_map(|row| {
                        let columns: Vec<_> = context
                            .columns()
                            .iter()
                            .zip(row.columns(context).ok()?)
                            .collect();
                        let recipient_type = match columns
                            .iter()
                            .find_map(|(col, value)| {
                                if col.prop_id() == 0x0C15 {
                                    Some((value.as_ref(), col.prop_type()))
                                } else {
                                    None
                                }
                            })
                            .and_then(|(value, prop_type)| {
                                self.pst_store
                                    .read_table_column(recipient_table, value?, prop_type)
                                    .ok()
                            })? {
                            PropertyValue::Integer32(value) => value,
                            _ => return None,
                        };
                        let display_name = match columns
                            .iter()
                            .find_map(|(col, value)| {
                                if col.prop_id() == 0x3001 {
                                    Some((value.as_ref(), col.prop_type()))
                                } else {
                                    None
                                }
                            })
                            .and_then(|(value, prop_type)| {
                                self.pst_store
                                    .read_table_column(recipient_table, value?, prop_type)
                                    .ok()
                            })? {
                            PropertyValue::String8(value) => value.to_string(),
                            PropertyValue::Unicode(value) => value.to_string(),
                            _ => return None,
                        };

                        match recipient_type {
                            MAPI_TO => Some(Recipient::To(display_name)),
                            MAPI_CC => Some(Recipient::Cc(display_name)),
                            MAPI_BCC => Some(Recipient::Bcc(display_name)),
                            _ => None,
                        }
                    })
                    .collect()
            })
            .as_slice())
    }
}

enum Body {
    Text(String),
    Html(String),
}

const MAPI_TO: i32 = 1;
const MAPI_CC: i32 = 2;
const MAPI_BCC: i32 = 3;

#[derive(Debug)]
enum Recipient {
    To(String),
    Cc(String),
    Bcc(String),
}

enum Attachment {
    File(Vec<u8>),
    Message(Message),
}

struct App {
    ipm_sub_tree: IpmSubTree,
}

#[derive(Debug, Default, PartialEq)]
enum Pane {
    #[default]
    Folders,
    Messages,
}

#[derive(Debug, Default)]
struct AppState {
    exit: bool,
    folder_path: Vec<usize>,
    current_pane: Pane,
    folder_state: ListState,
    message_state: ListState,
}

impl AppState {
    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_events(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        let list_state = match &self.current_pane {
            Pane::Folders => &mut self.folder_state,
            Pane::Messages => &mut self.message_state,
        };

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.exit(),
            KeyCode::Tab | KeyCode::BackTab => {
                self.current_pane = match self.current_pane {
                    Pane::Folders => Pane::Messages,
                    Pane::Messages => Pane::Folders,
                }
            }
            KeyCode::Char('h') | KeyCode::Left => self.go_back(),
            KeyCode::Char('j') | KeyCode::Down => list_state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => list_state.select_previous(),
            KeyCode::Char('l') | KeyCode::Right => {
                if self.current_pane == Pane::Folders {
                    self.change_folder(self.folder_state.selected());
                }
            }
            KeyCode::Char('g') | KeyCode::Home => list_state.select_first(),
            KeyCode::Char('G') | KeyCode::End => list_state.select_last(),
            _ => {}
        }
    }

    fn change_folder(&mut self, selected: Option<usize>) {
        if let Some(index) = selected {
            self.folder_path.push(index);
            self.folder_state = Default::default();
            self.message_state = Default::default();
        }
    }

    fn go_back(&mut self) {
        if self.folder_path.pop().is_some() {
            self.folder_state = Default::default();
            self.message_state = Default::default();
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl App {
    fn new(ipm_sub_tree: IpmSubTree) -> Self {
        Self { ipm_sub_tree }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut state = AppState::default();
        while !state.exit {
            terminal.draw(|frame| self.draw(frame, &mut state))?;
            state.handle_events()?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame, state: &mut AppState) {
        frame.render_stateful_widget(self, frame.area(), state);
    }
}

impl StatefulWidget for &App {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let block = Block::bordered().border_set(border::THICK);
        block.render(area, buf);

        let [folder_list, right_side] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)])
                .areas(area);
        let [message_list, reading_pane] =
            Layout::vertical([Constraint::Percentage(50); 2]).areas(right_side);
        let block = Block::bordered().border_set(border::Set {
            top_left: symbols::line::THICK_HORIZONTAL_DOWN,
            bottom_left: symbols::line::THICK_VERTICAL_RIGHT,
            bottom_right: symbols::line::THICK_VERTICAL_LEFT,
            ..border::THICK
        });
        block.render(message_list, buf);

        let block = Block::new()
            .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
            .border_set(border::Set {
                bottom_left: symbols::line::THICK_HORIZONTAL_UP,
                ..border::THICK
            });
        block.render(reading_pane, buf);

        let mut title = self.ipm_sub_tree.display_name();
        let mut current_folder: Option<&Folder> = None;
        for &index in &state.folder_path {
            current_folder = current_folder
                .and_then(|folder| folder.sub_folders().ok())
                .or_else(|| self.ipm_sub_tree.root_folders().ok())
                .and_then(|folders| folders.get(index));

            let Some(current_folder) = current_folder else {
                break;
            };

            title = format!("{title} > {}", current_folder.name);
        }

        let title = Line::from(title.bold());
        title.render(
            area.inner(Margin {
                horizontal: 3,
                vertical: 0,
            }),
            buf,
        );

        let sub_folders = current_folder
            .map(|folder| folder.sub_folders())
            .unwrap_or_else(|| self.ipm_sub_tree.root_folders())
            .unwrap_or(&[]);

        let folder_list = folder_list.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        StatefulWidget::render(
            List::new(sub_folders.iter().map(|folder| folder.name.as_str()))
                .style(Style::new().white())
                .highlight_style(Style::new().bold().blue()),
            folder_list,
            buf,
            &mut state.folder_state,
        );

        let messages = state
            .folder_state
            .selected()
            .and_then(|index| sub_folders.get(index))
            .and_then(|folder| folder.messages().ok())
            .unwrap_or(&[]);

        let message_list = message_list.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        StatefulWidget::render(
            List::new(messages.iter().map(|message| {
                message
                    .subject()
                    .ok()
                    .flatten()
                    .unwrap_or("(no subject)".to_string())
            }))
            .style(Style::new().white())
            .highlight_style(Style::new().bold().blue()),
            message_list,
            buf,
            &mut state.message_state,
        );

        let preview = state
            .message_state
            .selected()
            .and_then(|index| messages.get(index))
            .and_then(|message| message.full_message().ok())
            .and_then(|message| {
                message
                    .properties()
                    .get(0x1000)
                    .and_then(|value| match value {
                        PropertyValue::String8(value) => Some(value.to_string()),
                        PropertyValue::Unicode(value) => Some(value.to_string()),
                        _ => None,
                    })
                    .or_else(|| {
                        message
                            .properties()
                            .get(0x1013)
                            .and_then(|value| match value {
                                PropertyValue::Binary(value) => {
                                    match message.properties().get(0x3FDE) {
                                        Some(PropertyValue::Integer32(cpid)) => {
                                            let code_page = u16::try_from(*cpid).ok()?;
                                            encoding::decode_html_body(value.buffer(), code_page)
                                        }
                                        _ => None,
                                    }
                                }
                                PropertyValue::String8(value) => Some(value.to_string()),
                                PropertyValue::Unicode(value) => Some(value.to_string()),
                                _ => None,
                            })
                    })
                    .or_else(|| {
                        message
                            .properties()
                            .get(0x1009)
                            .and_then(|value| match value {
                                PropertyValue::Binary(value) => {
                                    encoding::decode_rtf_compressed(value.buffer())
                                }
                                _ => None,
                            })
                    })
            })
            .unwrap_or_else(|| "Hello, World!".to_string());

        let reading_pane = Rect {
            y: reading_pane.y,
            ..reading_pane.inner(Margin {
                horizontal: 1,
                vertical: 1,
            })
        };
        Paragraph::new(preview)
            .style(Style::new().white())
            .render(reading_pane, buf);
    }
}

fn main() -> anyhow::Result<()> {
    let args = args::Args::try_parse()?;
    let pst = UnicodePstFile::open(&args.file).unwrap();
    let ipm_sub_tree = IpmSubTree::new(pst);

    let mut terminal = ratatui::init();
    let app_result = App::new(ipm_sub_tree).run(&mut terminal);
    ratatui::restore();

    Ok(app_result?)
}
