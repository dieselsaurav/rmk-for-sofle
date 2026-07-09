#![no_main]
#![no_std]

use rmk::macros::rmk_central;
use sofle_rmk::OledDisplayController;

#[rmk_central]
mod keyboard_central {
    use embedded_graphics::text::Alignment;
    use sofle_rmk::Position;
    use sofle_rmk::Widget;

    #[controller(event)]
    fn oled_screen() -> OledDisplayController {
        OledDisplayController::new(
            p.TWISPI1,
            p.P0_17,
            p.P0_20,
            [
                Widget::new_layer(Alignment::Left, Position::Left | Position::Top),
                Widget::new_connection_type(Alignment::Right, Position::Right | Position::Top),
                Widget::new_pheripheral_status(Alignment::Left, Position::Left | Position::Bottom),
            ],
        )
    }
}
