use super::ActionButton;
use smithay::utils::{Logical, Point, Rectangle, Size};

pub const PREVIEW_SIZE: (i32, i32) = (300, 200);

/// Master on the left, vertical stack on the right
pub fn master_slave_layout(
    screen: Size<i32, Logical>,
    count: usize,
    gap: i32,
) -> Vec<Rectangle<i32, Logical>> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![Rectangle::new(
            Point::from((gap, gap)),
            Size::from((screen.w - gap * 2, screen.h - gap * 2)),
        )];
    }

    let mut result = Vec::with_capacity(count);

    let master_w = (screen.w - gap * 3) / 2;
    let stack_x = gap * 2 + master_w;
    let stack_w = screen.w - stack_x - gap;

    // Master (Window 0)
    result.push(Rectangle::new(
        Point::from((gap, gap)),
        Size::from((master_w, screen.h - gap * 2)),
    ));

    // Slaves (Windows 1..N)
    let slave_count = (count - 1) as i32;
    let total_gaps = (slave_count - 1) * gap;
    let usable_h = screen.h - gap * 2 - total_gaps;
    let base_h = usable_h / slave_count;
    let mut remainder = usable_h % slave_count;
    let mut current_y = gap;

    for _ in 0..slave_count {
        let extra = if remainder > 0 {
            remainder -= 1;
            1
        } else {
            0
        };
        let h = base_h + extra;
        result.push(Rectangle::new(
            Point::from((stack_x, current_y)),
            Size::from((stack_w, h)),
        ));
        current_y += h + gap;
    }

    result
}

/// Action overlay buttons centered inside a picked-up window
pub fn get_action_buttons(
    window_rect: Rectangle<i32, Logical>,
) -> [(ActionButton, Rectangle<i32, Logical>); 3] {
    let btn_size = Size::from((64, 64));
    let gap = 16;
    let total_w = btn_size.w * 3 + gap * 2;
    let start_x = window_rect.loc.x + (window_rect.size.w - total_w) / 2;
    let btn_y = window_rect.loc.y + (window_rect.size.h - btn_size.h) / 2;

    [
        (
            ActionButton::Minimize,
            Rectangle::new(Point::from((start_x, btn_y)), btn_size),
        ),
        (
            ActionButton::Maximize,
            Rectangle::new(Point::from((start_x + btn_size.w + gap, btn_y)), btn_size),
        ),
        (
            ActionButton::Close,
            Rectangle::new(
                Point::from((start_x + (btn_size.w + gap) * 2, btn_y)),
                btn_size,
            ),
        ),
    ]
}

pub struct StashPreview {
    pub main_rect: Rectangle<i32, Logical>,
    pub close_button: Rectangle<i32, Logical>,
}

pub struct StashLayout {
    pub main_rect: Rectangle<i32, Logical>,
    pub previews: Vec<StashPreview>,
    pub restore_all: Rectangle<i32, Logical>,
    pub close_all: Rectangle<i32, Logical>,
}

pub fn calculate_stash_layout(screen: Size<i32, Logical>, count: usize) -> StashLayout {
    let padding = 16;
    let gap = 16;
    let bottom_margin = 48;
    let preview_size = Size::from(PREVIEW_SIZE);

    let main_w = if count > 0 {
        (count as i32 * preview_size.w) + ((count as i32 - 1) * gap) + padding * 2
    } else {
        200
    };
    let main_h = preview_size.h + padding * 2;

    let main_loc = Point::new(
        (screen.w - main_w) / 2,
        screen.h - main_h - bottom_margin - 36,
    );
    let main_rect = Rectangle::new(main_loc, Size::new(main_w, main_h));

    let close_size = Size::new(20, 20);
    let mut previews = Vec::with_capacity(count);

    for i in 0..count {
        let card_x = main_loc.x + padding + (i as i32 * (preview_size.w + gap));
        let card_y = main_loc.y + padding;
        let main_rect = Rectangle::new(Point::new(card_x, card_y), preview_size);
        let close_button = Rectangle::new(
            Point::new(card_x + preview_size.w - close_size.w - 6, card_y + 6),
            close_size,
        );
        previews.push(StashPreview {
            main_rect,
            close_button,
        });
    }

    let pill_size = Size::new(100, 28);
    let pill_gap = 12;
    let total_pills_w = pill_size.w * 2 + pill_gap;
    let pills_x = (screen.w - total_pills_w) / 2;
    let pills_y = main_rect.loc.y + main_rect.size.h + 10;

    StashLayout {
        main_rect,
        previews,
        restore_all: Rectangle::new(Point::new(pills_x, pills_y), pill_size),
        close_all: Rectangle::new(
            Point::new(pills_x + pill_size.w + pill_gap, pills_y),
            pill_size,
        ),
    }
}
