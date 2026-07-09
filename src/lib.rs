#![no_std]

use core::fmt::Write;
use core::mem;

use defmt::unwrap;
use embassy_nrf::bind_interrupts;
use embassy_nrf::gpio::Pin;
use embassy_nrf::peripherals;
use embassy_nrf::twim;
use embassy_nrf::twim::Config;
use embassy_nrf::twim::Frequency;
use embassy_nrf::twim::Twim;
use embassy_nrf::Peri;
use embedded_graphics::geometry::AnchorX;
use embedded_graphics::geometry::AnchorY;
use embedded_graphics::mono_font::ascii::FONT_6X12;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::Dimensions;
use embedded_graphics::prelude::Point;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::Alignment;
use embedded_graphics::text::Baseline;
use embedded_graphics::text::Text;
use embedded_graphics::text::TextStyle;
use embedded_graphics::text::TextStyleBuilder;
use embedded_graphics::Drawable;
use enumflags2::bitflags;
use enumflags2::BitFlags;
use heapless::String;
use rmk::channel::ControllerSub;
use rmk::channel::CONTROLLER_CHANNEL;
use rmk::controller::Controller;
use rmk::event::ControllerEvent;
use rmk::state::ConnectionType;
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::mode::DisplayConfig;
use ssd1306::prelude::DisplayRotation;
use ssd1306::prelude::I2CInterface;
use ssd1306::size::DisplaySize128x32;
use ssd1306::I2CDisplayInterface;
use ssd1306::Ssd1306;

bind_interrupts!(struct Irqs {
    TWISPI1 => twim::InterruptHandler<peripherals::TWISPI1>;
});

type Display<'d> =
    Ssd1306<I2CInterface<Twim<'d>>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>;

pub struct OledDisplayController<'d, 'f, const N: usize> {
    sub: ControllerSub,
    display: Display<'d>,
    display_bbox: Rectangle,
    widgets: [Widget; N],
    text_style: MonoTextStyle<'f, BinaryColor>,
}

impl<'d, 'f, const N: usize> OledDisplayController<'d, 'f, N> {
    pub fn new(
        twim: Peri<'d, peripherals::TWISPI1>,
        sda: Peri<'d, impl Pin>,
        scl: Peri<'d, impl Pin>,
        widgets: [Widget; N],
    ) -> Self {
        let mut twim_config = Config::default();
        twim_config.frequency = Frequency::K400;
        twim_config.sda_high_drive = true;
        twim_config.sda_pullup = true;
        twim_config.scl_high_drive = true;
        twim_config.scl_pullup = true;

        let i2c = Twim::new(twim, Irqs, sda, scl, twim_config, &mut []);

        let interface = I2CDisplayInterface::new(i2c);
        let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

        let _ = display.init();
        // I could clear the display buffer when it initializes, so it doesn't have a bunch of
        // noise on it, but I found it pretty nice.
        // let _ = display.clear_buffer();

        let display_bbox = display.bounding_box();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X12)
            .text_color(BinaryColor::On)
            .build();

        Self {
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            display,
            display_bbox,
            widgets,
            text_style,
        }
    }
}

impl<const N: usize> Controller for OledDisplayController<'_, '_, N> {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        if !self
            .widgets
            .iter()
            .map(|widget| &widget.w_type)
            .map(WidgetType::event_discriminant)
            .any(|discriminant| discriminant == mem::discriminant(&event))
        {
            return;
        }

        self.widgets
            .iter_mut()
            .for_each(|widget| widget.update(event));

        self.display.clear_buffer();
        self.widgets
            .iter()
            .for_each(|widget| widget.draw(&mut self.display, self.display_bbox, self.text_style));

        let _ = self.display.flush();
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}

#[bitflags(default = Left | Top)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum Position {
    Left,
    Right,
    Top,
    Bottom,
}

impl Position {
    fn display_point(position: BitFlags<Self>, display_bbox: Rectangle) -> Point {
        position
            .iter()
            .map(|position| match position {
                Position::Left => (display_bbox.anchor_x(AnchorX::Left), 0),
                Position::Right => (display_bbox.anchor_x(AnchorX::Right), 0),
                Position::Top => (0, display_bbox.anchor_y(AnchorY::Top)),
                Position::Bottom => (0, display_bbox.anchor_y(AnchorY::Bottom)),
            })
            .fold(Point::zero(), |point, (x, y)| point + Point::new(x, y))
    }

    fn baseline(position: BitFlags<Self>) -> Baseline {
        match position {
            _ if position.contains(Position::Top) => Baseline::Top,
            _ if position.contains(Position::Bottom) => Baseline::Bottom,
            _ => Baseline::Top,
        }
    }
}

macro_rules! fn_new_widget_type {
    ($fn_name:ident, $w_type:expr) => {
        paste::item! {
            pub fn [< new_ $fn_name >] (alignment: Alignment, position: BitFlags<Position>) -> Self {
                Self {
                    w_type: $w_type,
                    position,
                    text_style: TextStyleBuilder::new()
                        .alignment(alignment)
                        .baseline(Position::baseline(position))
                        .build(),
                }
            }
        }
    };
}

pub struct Widget {
    w_type: WidgetType,
    position: BitFlags<Position>,
    text_style: TextStyle,
}

impl Widget {
    fn_new_widget_type!(layer, WidgetType::Layer(0));
    fn_new_widget_type!(connection_type, WidgetType::ConnType(ConnectionType::Usb));
    fn_new_widget_type!(pheripheral_status, WidgetType::PheripheralStatus(false));

    fn update(&mut self, event: ControllerEvent) {
        match (event, &mut self.w_type) {
            (ControllerEvent::Layer(layer), WidgetType::Layer(ref mut current)) => *current = layer,
            (ControllerEvent::ConnectionType(conn), WidgetType::ConnType(ref mut conn_type)) => {
                *conn_type = ConnectionType::from(conn)
            }
            (
                ControllerEvent::SplitPeripheral(_, status),
                WidgetType::PheripheralStatus(ref mut connected),
            ) => *connected = status,

            _ => {}
        }
    }

    fn draw<'f>(
        &self,
        display: &mut Display<'_>,
        display_bbox: Rectangle,
        char_style: MonoTextStyle<'f, BinaryColor>,
    ) {
        let position = Position::display_point(self.position, display_bbox);

        match &self.w_type {
            WidgetType::Layer(current) => {
                let mut buf = String::<20>::new();
                let _ = write!(&mut buf, "layer {}", current);
                let _ = Text::with_text_style(buf.as_str(), position, char_style, self.text_style)
                    .draw(display);
            }

            WidgetType::ConnType(conn_type) => {
                let connection_type = match conn_type {
                    ConnectionType::Usb => "USB",
                    ConnectionType::Ble => "BLE",
                };

                let _ =
                    Text::with_text_style(connection_type, position, char_style, self.text_style)
                        .draw(display);
            }

            WidgetType::PheripheralStatus(connected) => {
                let conn_status = match connected {
                    true => "connected",
                    false => "disconnected",
                };

                let _ = Text::with_text_style(conn_status, position, char_style, self.text_style)
                    .draw(display);
            }
        }
    }
}

enum WidgetType {
    Layer(u8),
    ConnType(ConnectionType),
    PheripheralStatus(bool),
}

impl WidgetType {
    /// Returns the [ControllerEvent] discriminant related to self.
    fn event_discriminant(&self) -> mem::Discriminant<ControllerEvent> {
        match self {
            Self::Layer(_) => mem::discriminant(&ControllerEvent::Layer(0)),
            Self::ConnType(_) => mem::discriminant(&ControllerEvent::ConnectionType(0)),
            Self::PheripheralStatus(_) => {
                mem::discriminant(&ControllerEvent::SplitPeripheral(0, false))
            }
        }
    }
}
