use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;

/// Represents a text decoration range with start and end line numbers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationRange {
    pub start_line: u32,
    pub end_line: u32,
}

/// Defines available decoration types with their visual properties
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DecorationType {
    FadedOverlay,
    ActiveLine,
}

/// Controls visual decorations in the text editor
#[derive(Debug)]
pub struct DecorationController {
    // Maps decoration types to their active ranges
    decorations: HashMap<DecorationType, Vec<DecorationRange>>,
    // Cache decoration type keys after registration
    decoration_type_keys: HashMap<DecorationType, String>,
}

impl DecorationController {
    /// Creates a new decoration controller
    pub fn new() -> Self {
        Self {
            decorations: HashMap::new(),
            decoration_type_keys: HashMap::new(),
        }
    }

    /// Registers decoration types with VSCode and caches their keys
    pub async fn register_decoration_types(&mut self) -> Result<()> {
        // Register FadedOverlay decoration type
        let faded_overlay_key = self
            .register_decoration_type(
                DecorationType::FadedOverlay,
                json!({
                    "backgroundColor": "rgba(255, 255, 0, 0.1)",
                    "isWholeLine": true
                }),
            )
            .await?;

        // Register ActiveLine decoration type
        let active_line_key = self
            .register_decoration_type(
                DecorationType::ActiveLine,
                json!({
                    "backgroundColor": "rgba(255, 255, 0, 0.3)",
                    "borderColor": "rgba(255, 255, 0, 0.5)",
                    "borderStyle": "solid",
                    "borderWidth": "1px",
                    "isWholeLine": true
                }),
            )
            .await?;

        // Cache the decoration type keys
        self.decoration_type_keys
            .insert(DecorationType::FadedOverlay, faded_overlay_key);
        self.decoration_type_keys
            .insert(DecorationType::ActiveLine, active_line_key);

        Ok(())
    }

    /// Applies decorations of a specific type to given ranges
    pub async fn set_decorations(
        &mut self,
        decoration_type: DecorationType,
        ranges: Vec<DecorationRange>,
    ) -> Result<()> {
        // Update internal state
        self.decorations
            .insert(decoration_type.clone(), ranges.clone());

        // Get decoration type key
        let type_key = self
            .decoration_type_keys
            .get(&decoration_type)
            .ok_or_else(|| anyhow::anyhow!("Decoration type not registered"))?;

        // Convert ranges to VSCode format
        let vscode_ranges = ranges
            .into_iter()
            .map(|range| {
                json!({
                    "start": { "line": range.start_line, "character": 0 },
                    "end": { "line": range.end_line, "character": 0 }
                })
            })
            .collect::<Vec<_>>();

        // Apply decorations through VSCode API
        self.apply_vscode_decorations(type_key, vscode_ranges)
            .await?;

        Ok(())
    }

    /// Clears all decorations of a specific type
    pub async fn clear_decorations(&mut self, decoration_type: DecorationType) -> Result<()> {
        self.set_decorations(decoration_type, vec![]).await
    }

    /// Merges overlapping or adjacent ranges for efficient decoration application
    fn merge_ranges(&self, mut ranges: Vec<DecorationRange>) -> Vec<DecorationRange> {
        if ranges.is_empty() {
            return ranges;
        }

        // Sort ranges by start line
        ranges.sort_by_key(|r| r.start_line);

        let mut merged = Vec::new();
        let mut current = ranges[0].clone();

        for range in ranges.into_iter().skip(1) {
            if range.start_line <= current.end_line + 1 {
                // Ranges overlap or are adjacent, merge them
                current.end_line = current.end_line.max(range.end_line);
            } else {
                // No overlap, push current range and start new one
                merged.push(current);
                current = range;
            }
        }
        merged.push(current);

        merged
    }

    /// Updates decorations for a specific range in the editor
    pub async fn update_range_decorations(
        &mut self,
        decoration_type: DecorationType,
        range: DecorationRange,
    ) -> Result<()> {
        let mut ranges = self
            .decorations
            .get(&decoration_type)
            .cloned()
            .unwrap_or_default();

        ranges.push(range);
        let merged = self.merge_ranges(ranges);
        self.set_decorations(decoration_type, merged).await
    }

    // Private helper methods
    async fn register_decoration_type(
        &self,
        decoration_type: DecorationType,
        options: serde_json::Value,
    ) -> Result<String> {
        // VSCode API call to create decoration type would go here
        // For now, return a placeholder key
        Ok(format!("decoration-{:?}", decoration_type))
    }

    async fn apply_vscode_decorations(
        &self,
        type_key: &str,
        ranges: Vec<serde_json::Value>,
    ) -> Result<()> {
        // VSCode API call to apply decorations would go here
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_ranges() {
        let controller = DecorationController::new();

        // Test overlapping ranges
        let ranges = vec![
            DecorationRange {
                start_line: 1,
                end_line: 3,
            },
            DecorationRange {
                start_line: 2,
                end_line: 4,
            },
            DecorationRange {
                start_line: 6,
                end_line: 8,
            },
        ];

        let merged = controller.merge_ranges(ranges);
        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged[0],
            DecorationRange {
                start_line: 1,
                end_line: 4
            }
        );
        assert_eq!(
            merged[1],
            DecorationRange {
                start_line: 6,
                end_line: 8
            }
        );
    }
}
