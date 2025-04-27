/// This file provides mock implementations for vscode functionality
/// used in the integration code. We'll keep a minimal interface
/// just to satisfy compilation.

pub struct DiffEditorOptions {
    pub ignore_trim_whitespace: bool,
    pub render_side_by_side: bool,
    pub original_editor_title: Option<String>,
    pub modified_editor_title: Option<String>,
}

impl Default for DiffEditorOptions {
    fn default() -> Self {
        Self {
            ignore_trim_whitespace: false,
            render_side_by_side: true,
            original_editor_title: None,
            modified_editor_title: None,
        }
    }
}