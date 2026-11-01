//! Asking for dialogs: the Attribute Definition dialog, the Enhanced
//! Attribute Editor and the Block Definition dialog.

use od_core::block::attribute_draft::{DefinitionDraft, EditorDraft};
use od_core::block::block_draft::BlockDraft;

use super::{Cancelled, Editor, Options, Prompt, Reply, Wants};
use crate::BlockDialogButton;

/// What the user did with the Attribute Editor.
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeEditorAnswer {
    /// OK: make the changes and close.
    Ok(EditorDraft),
    /// Apply: make the changes and keep it open.
    Apply(EditorDraft),
    /// Select block: choose another block reference. What was typed comes
    /// back, to be offered again if the user backs out of the pick.
    SelectBlock(EditorDraft),
}

impl Editor {
    /// Open the Attribute Definition dialog on `draft` and wait for OK.
    /// Cancel is `Err(Cancelled)`.
    pub fn ask_attribute_definition(
        &self,
        draft: DefinitionDraft,
    ) -> Result<DefinitionDraft, Cancelled> {
        let options = Options::<()>::new("Attribute Definition");
        let mut prompt = Prompt::new(Wants::AttributeDefinitionDialog, &options, None);
        prompt.attribute_definition = Some(Box::new(draft));
        match self.ask(prompt)? {
            Reply::AttributeDefinition(draft) => Ok(*draft),
            _ => Err(Cancelled),
        }
    }

    /// Open the Attribute Editor on `draft` and wait for what the
    /// user does with it. Cancel is `Err(Cancelled)`.
    pub fn ask_attribute_editor(
        &self,
        draft: EditorDraft,
    ) -> Result<AttributeEditorAnswer, Cancelled> {
        let options = Options::<()>::new("Attribute Editor");
        let mut prompt = Prompt::new(Wants::AttributeEditorDialog, &options, None);
        prompt.attribute_editor = Some(Box::new(draft));
        match self.ask(prompt)? {
            Reply::AttributeEditor(draft, true) => Ok(AttributeEditorAnswer::Ok(*draft)),
            Reply::AttributeEditor(draft, false) => Ok(AttributeEditorAnswer::Apply(*draft)),
            Reply::AttributeEditorSelect(draft) => Ok(AttributeEditorAnswer::SelectBlock(*draft)),
            _ => Err(Cancelled),
        }
    }

    /// Open the Block Definition dialog on `draft` and wait for a button.
    /// Cancel is `Err(Cancelled)`.
    pub fn ask_block_dialog(
        &self,
        draft: BlockDraft,
    ) -> Result<(BlockDialogButton, BlockDraft), Cancelled> {
        let options = Options::<()>::new("Block Definition");
        let mut prompt = Prompt::new(Wants::BlockDialog, &options, None);
        prompt.block_dialog = Some(Box::new(draft));
        match self.ask(prompt)? {
            Reply::BlockDialog(draft, button) => Ok((button, *draft)),
            _ => Err(Cancelled),
        }
    }
}
