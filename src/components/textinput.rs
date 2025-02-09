use crate::{
    components::{
        popup_paragraph, visibility_blocking, CommandBlocking,
        CommandInfo, Component, DrawableComponent,
    },
    keys::SharedKeyConfig,
    strings,
    ui::{self, style::SharedTheme},
};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tui::{
    backend::Backend, layout::Rect, style::Modifier, text::Span,
    widgets::Clear, Frame,
};

/// primarily a subcomponet for user input of text (used in `CommitComponent`)
pub struct TextInputComponent {
    title: String,
    default_msg: String,
    msg: String,
    visible: bool,
    theme: SharedTheme,
    key_config: SharedKeyConfig,
    cursor_position: usize,
}

impl TextInputComponent {
    ///
    pub fn new(
        theme: SharedTheme,
        key_config: SharedKeyConfig,
        title: &str,
        default_msg: &str,
    ) -> Self {
        Self {
            msg: String::default(),
            visible: false,
            theme,
            key_config,
            title: title.to_string(),
            default_msg: default_msg.to_string(),
            cursor_position: 0,
        }
    }

    /// Clear the `msg`.
    pub fn clear(&mut self) {
        self.msg.clear();
        self.cursor_position = 0;
    }

    /// Get the `msg`.
    pub const fn get_text(&self) -> &String {
        &self.msg
    }

    /// Move the cursor right one char.
    fn incr_cursor(&mut self) {
        if let Some(pos) = self.next_char_position() {
            self.cursor_position = pos;
        }
    }

    /// Move the cursor left one char.
    fn decr_cursor(&mut self) {
        let mut index = self.cursor_position.saturating_sub(1);
        while index > 0 && !self.msg.is_char_boundary(index) {
            index -= 1;
        }
        self.cursor_position = index;
    }

    /// Get the position of the next char, or, if the cursor points
    /// to the last char, the `msg.len()`.
    /// Returns None when the cursor is already at `msg.len()`.
    fn next_char_position(&self) -> Option<usize> {
        if self.cursor_position >= self.msg.len() {
            return None;
        }
        let mut index = self.cursor_position.saturating_add(1);
        while index < self.msg.len()
            && !self.msg.is_char_boundary(index)
        {
            index += 1;
        }
        Some(index)
    }

    fn backspace(&mut self) {
        if self.cursor_position > 0 {
            self.decr_cursor();
            self.msg.remove(self.cursor_position);
        }
    }

    /// Set the `msg`.
    pub fn set_text(&mut self, msg: String) {
        self.msg = msg;
        self.cursor_position = 0;
    }

    /// Set the `title`.
    pub fn set_title(&mut self, t: String) {
        self.title = t;
    }

    fn get_draw_text(&self) -> Vec<Span> {
        let style = self.theme.text(true, false);

        let mut txt = Vec::new();
        // The portion of the text before the cursor is added
        // if the cursor is not at the first character.
        if self.cursor_position > 0 {
            txt.push(Span::styled(
                &self.msg[..self.cursor_position],
                style,
            ));
        }

        let cursor_str = self
            .next_char_position()
            // if the cursor is at the end of the msg
            // a whitespace is used to underline
            .map_or(" ", |pos| &self.msg[self.cursor_position..pos]);

        if cursor_str == "\n" {
            txt.push(Span::styled(
                "\u{21b5}",
                self.theme
                    .text(false, false)
                    .add_modifier(Modifier::UNDERLINED),
            ));
        }

        txt.push(Span::styled(
            cursor_str,
            style.add_modifier(Modifier::UNDERLINED),
        ));

        // The final portion of the text is added if there are
        // still remaining characters.
        if let Some(pos) = self.next_char_position() {
            if pos < self.msg.len() {
                txt.push(Span::styled(&self.msg[pos..], style));
            }
        }

        txt
    }
}

impl DrawableComponent for TextInputComponent {
    fn draw<B: Backend>(
        &self,
        f: &mut Frame<B>,
        _rect: Rect,
    ) -> Result<()> {
        if self.visible {
            let txt = if self.msg.is_empty() {
                vec![Span::styled(
                    self.default_msg.as_str(),
                    self.theme.text(false, false),
                )]
            } else {
                self.get_draw_text()
            };

            let area = ui::centered_rect(60, 20, f.size());
            let area = ui::rect_min(10, 3, area);

            f.render_widget(Clear, area);
            f.render_widget(
                popup_paragraph(
                    self.title.as_str(),
                    txt,
                    &self.theme,
                    true,
                ),
                area,
            );
        }

        Ok(())
    }
}

impl Component for TextInputComponent {
    fn commands(
        &self,
        out: &mut Vec<CommandInfo>,
        _force_all: bool,
    ) -> CommandBlocking {
        out.push(
            CommandInfo::new(
                strings::commands::close_popup(&self.key_config),
                true,
                self.visible,
            )
            .order(1),
        );
        visibility_blocking(self)
    }

    fn event(&mut self, ev: Event) -> Result<bool> {
        if self.visible {
            if let Event::Key(e) = ev {
                if e == self.key_config.exit_popup {
                    self.hide();
                    return Ok(true);
                }

                let is_ctrl =
                    e.modifiers.contains(KeyModifiers::CONTROL);

                match e.code {
                    KeyCode::Char(c) if !is_ctrl => {
                        self.msg.insert(self.cursor_position, c);
                        self.incr_cursor();
                        return Ok(true);
                    }
                    KeyCode::Delete => {
                        if self.cursor_position < self.msg.len() {
                            self.msg.remove(self.cursor_position);
                        }
                        return Ok(true);
                    }
                    KeyCode::Backspace => {
                        self.backspace();
                        return Ok(true);
                    }
                    KeyCode::Left => {
                        self.decr_cursor();
                        return Ok(true);
                    }
                    KeyCode::Right => {
                        self.incr_cursor();
                        return Ok(true);
                    }
                    KeyCode::Home => {
                        self.cursor_position = 0;
                        return Ok(true);
                    }
                    KeyCode::End => {
                        self.cursor_position = self.msg.len();
                        return Ok(true);
                    }
                    _ => (),
                };
            }
        }
        Ok(false)
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn hide(&mut self) {
        self.visible = false
    }

    fn show(&mut self) -> Result<()> {
        self.visible = true;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui::style::Style;

    #[test]
    fn test_smoke() {
        let mut comp = TextInputComponent::new(
            SharedTheme::default(),
            SharedKeyConfig::default(),
            "",
            "",
        );

        comp.set_text(String::from("a\nb"));

        assert_eq!(comp.cursor_position, 0);

        comp.incr_cursor();
        assert_eq!(comp.cursor_position, 1);

        comp.decr_cursor();
        assert_eq!(comp.cursor_position, 0);
    }

    #[test]
    fn text_cursor_initial_position() {
        let mut comp = TextInputComponent::new(
            SharedTheme::default(),
            SharedKeyConfig::default(),
            "",
            "",
        );
        let theme = SharedTheme::default();
        let underlined = theme
            .text(true, false)
            .add_modifier(Modifier::UNDERLINED);

        comp.set_text(String::from("a"));

        let txt = comp.get_draw_text();

        assert_eq!(txt.len(), 1);
        assert_eq!(get_text(&txt[0]), Some("a"));
        assert_eq!(get_style(&txt[0]), Some(&underlined));
    }

    #[test]
    fn test_cursor_second_position() {
        let mut comp = TextInputComponent::new(
            SharedTheme::default(),
            SharedKeyConfig::default(),
            "",
            "",
        );
        let theme = SharedTheme::default();
        let underlined = theme
            .text(true, false)
            .add_modifier(Modifier::UNDERLINED);

        let not_underlined = Style::default();

        comp.set_text(String::from("a"));
        comp.incr_cursor();

        let txt = comp.get_draw_text();

        assert_eq!(txt.len(), 2);
        assert_eq!(get_text(&txt[0]), Some("a"));
        assert_eq!(get_style(&txt[0]), Some(&not_underlined));
        assert_eq!(get_text(&txt[1]), Some(" "));
        assert_eq!(get_style(&txt[1]), Some(&underlined));
    }

    #[test]
    fn test_visualize_newline() {
        let mut comp = TextInputComponent::new(
            SharedTheme::default(),
            SharedKeyConfig::default(),
            "",
            "",
        );

        let theme = SharedTheme::default();
        let underlined = theme
            .text(false, false)
            .add_modifier(Modifier::UNDERLINED);

        comp.set_text(String::from("a\nb"));
        comp.incr_cursor();

        let txt = comp.get_draw_text();

        assert_eq!(txt.len(), 4);
        assert_eq!(get_text(&txt[0]), Some("a"));
        assert_eq!(get_text(&txt[1]), Some("\u{21b5}"));
        assert_eq!(get_style(&txt[1]), Some(&underlined));
        assert_eq!(get_text(&txt[2]), Some("\n"));
        assert_eq!(get_text(&txt[3]), Some("b"));
    }

    #[test]
    fn test_invisable_newline() {
        let mut comp = TextInputComponent::new(
            SharedTheme::default(),
            SharedKeyConfig::default(),
            "",
            "",
        );

        let theme = SharedTheme::default();
        let underlined = theme
            .text(true, false)
            .add_modifier(Modifier::UNDERLINED);

        comp.set_text(String::from("a\nb"));

        let txt = comp.get_draw_text();

        assert_eq!(txt.len(), 2);
        assert_eq!(get_text(&txt[0]), Some("a"));
        assert_eq!(get_style(&txt[0]), Some(&underlined));
        assert_eq!(get_text(&txt[1]), Some("\nb"));
    }

    fn get_text<'a>(t: &'a Span) -> Option<&'a str> {
        Some(&t.content)
    }

    fn get_style<'a>(t: &'a Span) -> Option<&'a Style> {
        Some(&t.style)
    }
}
