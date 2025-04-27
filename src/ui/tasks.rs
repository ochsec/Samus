use crate::task::Task;
use crate::ui::task_types::TaskOutput;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::sync::Arc;

#[derive(Debug)]
pub struct TaskView {
    tasks: Vec<Arc<Task>>,
    selected_index: Option<usize>,
    current_output: Option<TaskOutput>,
}

impl TaskView {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            selected_index: None,
            current_output: None,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(Arc::new(task));
    }

    pub fn select_next(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) if i + 1 < self.tasks.len() => i + 1,
            _ => 0,
        });
    }

    pub fn select_previous(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) if i > 0 => i - 1,
            _ => self.tasks.len() - 1,
        });
    }

    pub fn selected_task(&self) -> Option<Arc<Task>> {
        self.selected_index.and_then(|i| self.tasks.get(i).cloned())
    }

    pub fn update_output(&mut self, output: TaskOutput) {
        self.current_output = Some(output);
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        self.render_task_list(frame, chunks[0]);
        self.render_task_detail(frame, chunks[1]);
    }

    fn render_task_list(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let style = if Some(i) == self.selected_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{}: ", task.id), style),
                    Span::styled(&task.name, style),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().title("Tasks").borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(list, area);
    }

    fn render_task_detail(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().title("Task Detail").borders(Borders::ALL);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if let Some(task) = self.selected_task() {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Task info
                    Constraint::Length(3), // Resources
                    Constraint::Min(0),    // Output
                ])
                .split(inner_area);

            // Task info
            let info = Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("ID: "),
                    Span::styled(&task.id, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("Name: "),
                    Span::styled(&task.name, Style::default().fg(Color::Cyan)),
                ]),
            ]);
            frame.render_widget(info, chunks[0]);

            // Resources (Commented out since Task doesn't have resources field yet)
            let resources = Paragraph::new(Line::from(vec![Span::raw("No resources attached")]));
            frame.render_widget(resources, chunks[1]);

            // Output
            if let Some(output) = &self.current_output {
                let style = if output.success {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };

                let output_text = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled("Status: ", style),
                        Span::styled(if output.success { "Success" } else { "Failed" }, style),
                    ]),
                    Line::from(vec![Span::raw("")]), // Empty line
                    Line::from(vec![Span::raw(
                        output.message.as_deref().unwrap_or("No message"),
                    )]),
                ]);
                frame.render_widget(output_text, chunks[2]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Task;

    #[test]
    fn test_task_view_creation() {
        let view = TaskView::new();
        assert!(view.tasks.is_empty());
        assert!(view.selected_index.is_none());
        assert!(view.current_output.is_none());
    }

    #[test]
    fn test_task_selection() {
        let mut view = TaskView::new();

        // Add some test tasks
        view.add_task(Task::new("task1", "Task 1"));
        view.add_task(Task::new("task2", "Task 2"));

        // Test selection
        view.select_next();
        assert_eq!(view.selected_index, Some(0));

        view.select_next();
        assert_eq!(view.selected_index, Some(1));

        view.select_next();
        assert_eq!(view.selected_index, Some(0));

        view.select_previous();
        assert_eq!(view.selected_index, Some(1));
    }

    #[test]
    fn test_task_output_update() {
        let mut view = TaskView::new();
        let output = TaskOutput {
            success: true,
            message: Some("Test completed".to_string()),
        };

        view.update_output(output.clone());
        assert_eq!(
            view.current_output.as_ref().unwrap().message,
            output.message
        );
    }
}
