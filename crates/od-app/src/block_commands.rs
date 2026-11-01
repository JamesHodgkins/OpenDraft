//! The commands that make, place and describe blocks: BLOCK and INSERT, the
//! attribute commands, and their command-line (`-`) forms.

use od_commands::Command;

use crate::attdef_cmd::define_attribute;
use crate::attribute_dialog_cmd::{attdef_dialog, eattedit_dialog};
use crate::attribute_edit::edit_attributes;
use crate::attribute_sync_cmd::sync_attributes;
use crate::block_cmd::define_block;
use crate::block_dialog_cmd::define_block_dialog;
use crate::insert_cmd::insert_block;
use crate::state::AppState;

/// As `editor_command!` in `commands.rs`.
macro_rules! editor {
    ($id:literal, $label:literal, $name:literal, $run:path) => {
        Command::new($id, $label, |s: &mut AppState| s.start_command($name, $run))
            .checked(|s: &AppState| s.is_running($name))
            .transparent()
    };
}

/// The commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        editor!("block.create", "Create Block", "BLOCK", define_block_dialog),
        editor!("block.create_line", "-BLOCK", "-BLOCK", define_block),
        editor!("block.insert", "Insert Block", "INSERT", insert_block),
        editor!("block.attdef", "Define Attributes", "ATTDEF", attdef_dialog),
        editor!("block.attdef_line", "-ATTDEF", "-ATTDEF", define_attribute),
        editor!(
            "block.attedit",
            "Edit Attributes",
            "EATTEDIT",
            eattedit_dialog
        ),
        editor!(
            "block.attedit_line",
            "-ATTEDIT",
            "-ATTEDIT",
            edit_attributes
        ),
        editor!(
            "block.attsync",
            "Sync Attributes",
            "ATTSYNC",
            sync_attributes
        ),
    ]
}
