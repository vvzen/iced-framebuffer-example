// UI
use iced::theme::Theme;
use iced::widget::{button, column, container, image, row, text, text_input};
use iced::{Alignment, Element, Length, Sandbox, Settings};

// Color
use colstodian::spaces::{AcesCg, EncodedSrgb};
use colstodian::tonemap::{PerceptualTonemapper, PerceptualTonemapperParams, Tonemapper};
use colstodian::{color, Color, Display, Scene};

#[derive(Debug, Clone)]
pub enum ApplicationMessage {
    FileNameChanged(String),
    SaveFilePressed,
    RenderPressed,
}

struct ApplicationState {
    file_name: String,
    file_name_with_ext: String,
    rendered_image: image::Handle,
}

const FONT_BYTES: &[u8; 283684] = include_bytes!("../media/FiraCode-Medium.ttf");
const RENDER_BUFFER_WIDTH: usize = 1024;
const RENDER_BUFFER_HEIGHT: usize = 1024;
const RENDER_BUFFER_SIZE: usize = RENDER_BUFFER_WIDTH * RENDER_BUFFER_HEIGHT * 4;

/// Linear remap a value in one range into another range (no clamping)
pub fn fit_range(x: f32, imin: f32, imax: f32, omin: f32, omax: f32) -> f32 {
    (omax - omin) * (x - imin) / (imax - imin) + omin
}

// Sample function demostrating how to render a custom image
fn render_bg_image() -> [u8; RENDER_BUFFER_SIZE] {
    let mut linear_render_buffer = vec![0.0; RENDER_BUFFER_SIZE];

    // Render a in linear color space
    let mut index: usize = 0;
    for y in (0..RENDER_BUFFER_HEIGHT).rev() {
        for x in 0..RENDER_BUFFER_WIDTH {
            // Get normalized U,V coordinates as we move through the image
            let u = fit_range(x as f32, 0.0, RENDER_BUFFER_WIDTH as f32, 0.0, 1.0);
            let v = fit_range(y as f32, 0.0, RENDER_BUFFER_HEIGHT as f32, 0.0, 1.0);

            // Generate a gradient between two colors in AcesCG
            // TODO: Could we do this in LAB, and then convert to ACES CG ?
            let red = color::acescg::<Scene>(1.0, 0.0, 0.0);
            let green = color::acescg::<Scene>(0.0, 1.0, 0.0);
            let blue = color::acescg::<Scene>(0.0, 0.0, 1.0);
            let h_blended = red.blend(green, u);
            let v_blended = red.blend(blue, v);
            let final_color = h_blended.blend(v_blended, 0.5);

            let rendered_color =
                color::acescg::<Scene>(final_color.r, final_color.g, final_color.b);

            // R, G, B, A
            linear_render_buffer[index + 0] = rendered_color.r;
            linear_render_buffer[index + 1] = rendered_color.g;
            linear_render_buffer[index + 2] = rendered_color.b;
            linear_render_buffer[index + 3] = 1.0;

            index += 4;
        }
    }

    // Do the scene linear to display conversion
    let mut display_buffer: [u8; RENDER_BUFFER_SIZE] = [0; RENDER_BUFFER_SIZE];
    let it = std::iter::zip(
        linear_render_buffer.chunks_exact(4),
        display_buffer.chunks_exact_mut(4),
    );

    for (f32_pixel, u8_pixel) in it {
        // For the sake of simplicity and saving memory, our array is composed of f32
        // instead of colostodian Color structs. Here we recreate the colstodian struct
        // on the fly so we can do the conversion to 8bit sRGB and go to display referred
        // by applying default a SDR tone mapping
        let rendered_color = colstodian::color::acescg(f32_pixel[0], f32_pixel[1], f32_pixel[2]);

        // Use a standard Tonemap to go from ACEScg HDR to SDR
        let params = PerceptualTonemapperParams::default();
        let tonemapped: Color<AcesCg, Display> =
            PerceptualTonemapper::tonemap(rendered_color, params).convert();

        // Encode in sRGB so we're ready to display or write to an image
        let encoded = tonemapped.convert::<EncodedSrgb>();

        // Convert to 8bit
        let rgb: [u8; 3] = encoded.to_u8();
        let alpha = f32_pixel[3];

        // Can I avoid doing a copy here ?
        let rgba: [u8; 4] = [rgb[0], rgb[1], rgb[2], (255 as f32 * alpha) as u8];

        u8_pixel.copy_from_slice(&rgba);
    }

    display_buffer
}

impl Sandbox for ApplicationState {
    type Message = ApplicationMessage;

    fn new() -> Self {
        let file_name = String::from("sample_file");

        let buffer_data = render_bg_image();

        // Creates an image Handle containing the image pixels directly.
        // This function expects the input data to be provided as a Vec<u8> of RGBA pixels.
        let image = image::Handle::from_pixels(
            RENDER_BUFFER_WIDTH as u32,
            RENDER_BUFFER_HEIGHT as u32,
            buffer_data,
        );

        ApplicationState {
            file_name: file_name.clone(),
            file_name_with_ext: format!("{file_name}.exr"),
            rendered_image: image,
        }
    }

    fn title(&self) -> String {
        String::from("Iced Sample Render image App")
    }

    // Description of the UI
    fn view(&self) -> Element<Self::Message> {
        // This stores the image after it has been rendered
        let image_viewer = image::Viewer::new(self.rendered_image.clone()).min_scale(1.0);

        let rendered_image = container(image_viewer)
            .width(Length::Fill)
            .center_x()
            .max_height(512)
            .max_width(800);

        // Render button
        let render_button = button(
            text("Render")
                .width(Length::Fill)
                .horizontal_alignment(iced::alignment::Horizontal::Center),
        )
        .on_press(Self::Message::RenderPressed)
        .padding(10)
        .width(Length::Fill);

        // Save text field
        let file_name_input = text_input(
            "Your file name",
            &self.file_name,
            Self::Message::FileNameChanged,
        )
        .padding(10)
        .size(20);

        let save_button = button(
            text("Save")
                .width(Length::Fill)
                .horizontal_alignment(iced::alignment::Horizontal::Center),
        )
        .on_press(Self::Message::SaveFilePressed)
        .padding(10)
        .width(100);

        let content = column![
            row![rendered_image].padding(10).spacing(10),
            row![render_button].padding(10).spacing(10),
            row![file_name_input, save_button].padding(10).spacing(10),
        ]
        .max_width(800);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn update(&mut self, message: ApplicationMessage) {
        match message {
            ApplicationMessage::RenderPressed => {
                eprintln!("Rendering in the background...");
            }
            ApplicationMessage::FileNameChanged(new_name) => {
                eprintln!("New name: {new_name}");
                self.file_name = new_name;
                eprintln!("New file name: {}", self.file_name);
                self.file_name_with_ext = format!("{}.exr", self.file_name);
            }
            ApplicationMessage::SaveFilePressed => {
                eprintln!("Saving {} to disk..", self.file_name_with_ext);
            }
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn main() {
    let mut settings = Settings::default();
    settings.default_font = Some(FONT_BYTES);
    ApplicationState::run(settings).unwrap();
}
