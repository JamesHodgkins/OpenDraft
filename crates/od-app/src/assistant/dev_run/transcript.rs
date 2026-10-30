//! What the model was sent and what it did, written to a folder after a hand
//! run (`OD_DEV_TRANSCRIPT=folder`): a text file of every call and result,
//! and each picture the model was shown as a png file.

use std::fmt::Write as _;
use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use od_agent::{Content, Message, Part};

/// A result longer than this is cut in the text file.
const RESULT_AT_MOST: usize = 3000;

pub fn write(folder: &Path, messages: &[Message]) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut text = String::new();
    let mut pictures = 0;
    for message in messages {
        for part in &message.parts {
            match part {
                Part::Text(t) => {
                    let _ = writeln!(text, "{:?}: {t}\n", message.role);
                }
                Part::ToolCall(call) => {
                    let _ = writeln!(text, "CALL {} {}\n", call.name, call.arguments);
                }
                Part::ToolResult(result) => {
                    let error = if result.is_error { " (error)" } else { "" };
                    for content in &result.content {
                        match content {
                            Content::Text(t) => {
                                let shown: String = t.chars().take(RESULT_AT_MOST).collect();
                                let _ = writeln!(text, "RESULT{error} {shown}\n");
                            }
                            Content::Image(image) => {
                                pictures += 1;
                                let name = format!("picture-{pictures}.png");
                                let bytes =
                                    STANDARD.decode(&image.base64).map_err(|e| e.to_string())?;
                                std::fs::write(folder.join(&name), bytes)
                                    .map_err(|e| e.to_string())?;
                                let _ = writeln!(text, "PICTURE {name}: {}\n", image.label);
                            }
                        }
                    }
                }
                Part::Image(image) => {
                    let _ = writeln!(text, "IMAGE: {}\n", image.label);
                }
            }
        }
    }
    std::fs::write(folder.join("transcript.txt"), text).map_err(|e| e.to_string())
}
