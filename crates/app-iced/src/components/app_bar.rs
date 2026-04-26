use iced::{widget::{button, container::Style, text, Container, Row}, Color, Element, Length::Fill, Pixels};

pub struct AppBar<'a, Message: Clone + 'a> {
    title: &'a str,
    /// What message to send when the back button is clicked, if none, then the back button is disabled
    on_back_button: Option<Message>,
    height: Pixels,
}

impl<'a, Message: Clone + 'a> AppBar<'a, Message> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            on_back_button: None,
            height: 35.0.into(),
        }
    }

    pub fn on_back_button_maybe(mut self, message: Option<Message>) -> Self {
        self.on_back_button = message;
        self
    }

    pub fn on_back_button(self, message: Message) -> Self {
        self.on_back_button_maybe(Some(message))
    }

    pub fn with_height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into();
        self
    }

    pub fn height(&self) -> Pixels {
        self.height
    }
}

impl<'a, Message: Clone + 'a> From<AppBar<'a, Message>> for Element<'a, Message> {
    fn from(
        AppBar {
            title,
            on_back_button,
            height,
        }: AppBar<'a, Message>,
    ) -> Self {
        let mut row: Row<'a, Message> = Row::new().width(Fill).height(height);
        if let Some(on_back_button) = on_back_button {
            row = row.push(
                button("back") //  TODO: user icon
                    .on_press(on_back_button),
            )
        }
        let row = row.push(text(title).center().width(Fill).height(Fill));
        let row = Container::new(row).style(|_theme| Style {
            text_color: Some(Color::WHITE),
            background: Some(Color::from_rgb8(49, 116, 219).into()),
            ..Style::default()
        });
        row.into()
    }
}
