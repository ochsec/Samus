use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, layout::Rect,
};

/// ASCII representation of Samus title logo
const SAMUS_LOGO: &str = r#"
  ███████╗ █████╗ ███╗   ███╗██╗   ██╗███████╗
  ██╔════╝██╔══██╗████╗ ████║██║   ██║██╔════╝
  ███████╗███████║██╔████╔██║██║   ██║███████╗
  ╚════██║██╔══██║██║╚██╔╝██║██║   ██║╚════██║
  ███████║██║  ██║██║ ╚═╝ ██║╚██████╔╝███████║
  ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝ ╚═════╝ ╚══════╝
"#;

/// Renders the Samus logo on startup
pub fn render_logo(f: &mut Frame, area: Rect) {
    let logo_lines: Vec<Line> = SAMUS_LOGO
        .lines()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::Green))))
        .collect();

    let logo = Paragraph::new(logo_lines)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default());

    f.render_widget(logo, area);
}

/// Renders a pixel art version of the Samus logo
/// This uses the PNG image converted to ASCII representation
pub fn render_pixel_logo(f: &mut Frame, area: Rect) {
    // ASCII art representation of the samus_title.png
    let pixel_logo = r#"
 ███████  █████  ████████ ████████  ██    ██  ███████ 
██        ██   ██    ██      ██     ██    ██ ██      
 █████    ███████    ██      ██     ██    ██  █████  
     ██   ██   ██    ██      ██     ██    ██      ██ 
███████   ██   ██    ██      ██      ██████  ███████ 
"#;

    let pixel_lines: Vec<Line> = pixel_logo
        .lines()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::Green))))
        .collect();

    let logo = Paragraph::new(pixel_lines)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default());

    f.render_widget(logo, area);
}