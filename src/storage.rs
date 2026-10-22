use std::{fs, io::Write, rc::Rc};

use snafu::ResultExt;
use tracing::{Level, event};

use crate::{Result, chat::ChatSession};

pub trait ChatStorage {
    fn save(&self, session: Rc<dyn ChatSession>) -> Result<bool>;
}

pub struct DefaultChatStorage;

impl ChatStorage for DefaultChatStorage {
    fn save(&self, session: Rc<dyn ChatSession>) -> Result<bool> {
        let storage_dir = crate::get_project_dirs().data_dir().join("sessions");
        fs::create_dir_all(&storage_dir).with_whatever_context(|_| {
            format!(
                "creating one or more directories: {}",
                storage_dir.display()
            )
        })?;

        let session_name = storage_dir.join(format!("{}.jsonl", session.get_id()));
        let session_json = session.get_json();
        if session.needs_saving() {
            let out = fs::File::create(&session_name)
                .with_whatever_context(|_| format!("creating file {}", session_name.display()))?;

            // TODO the first line should actually be some sort of
            // header, which has the session ID and the session type.
            // And perhaps the session's lineage.
            // And perhaps the model? Since we're storing reasoning details,
            // does this tie our chat sessions to particular models?

            for session_item in &session_json {
                serde_json::to_writer(&out, session_item).with_whatever_context(|_| {
                    format!("writing file {}", session_name.display())
                })?;
                writeln!(&out).with_whatever_context(|_| {
                    format!("writing file {}", session_name.display())
                })?;
            }
            Ok(true)
        } else {
            event!(
                Level::TRACE,
                "session {} is empty; not saving",
                session.get_id()
            );
            Ok(false)
        }
    }
}
