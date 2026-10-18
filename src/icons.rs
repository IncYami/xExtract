pub const FILM_STRIP: &str = "\u{e792}";
pub const GEAR: &str = "\u{e270}";
pub const PLAY: &str = "\u{e3d0}";
pub const STOP: &str = "\u{e46c}";
pub const FOLDER_OPEN: &str = "\u{e256}";
pub const TRASH_SIMPLE: &str = "\u{e4a8}";
pub const FOLDER_PLUS: &str = "\u{e258}";
pub const FILE_PLUS: &str = "\u{e236}";
pub const X: &str = "\u{e4f6}";
pub const CLOCK: &str = "\u{e19a}";
pub const TERMINAL_WINDOW: &str = "\u{eae8}";
pub const COPY: &str = "\u{e1ca}";
pub const HOURGLASS: &str = "\u{e2b2}";
pub const MAGNIFYING_GLASS: &str = "\u{e30c}";
pub const CHECK_CIRCLE: &str = "\u{e184}";
pub const ARROW_CLOCKWISE: &str = "\u{e036}";
pub const X_CIRCLE: &str = "\u{e4f8}";

const REGULAR_FONT_NAME: &str = "phosphor_regular";
const FILL_FONT_NAME: &str = "phosphor_fill";

const REGULAR_FONT: &[u8] = include_bytes!("../assets/fonts/Phosphor-Regular-2.1.2.ttf");
const FILL_FONT: &[u8] = include_bytes!("../assets/fonts/Phosphor-Fill-2.1.2.ttf");

pub fn install_fonts(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    add_font_as_family(&mut fonts, REGULAR_FONT_NAME, REGULAR_FONT);
    if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        let index = font_keys.len().min(1);
        font_keys.insert(index, REGULAR_FONT_NAME.to_owned());
    }

    add_font_as_family(&mut fonts, FILL_FONT_NAME, FILL_FONT);
    context.set_fonts(fonts);
}

fn add_font_as_family(fonts: &mut egui::FontDefinitions, name: &str, bytes: &'static [u8]) {
    fonts
        .font_data
        .insert(name.to_owned(), egui::FontData::from_static(bytes).into());

    let mut font_keys = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let index = font_keys.len().min(1);
    font_keys.insert(index, name.to_owned());
    fonts
        .families
        .insert(egui::FontFamily::Name(name.into()), font_keys);
}
