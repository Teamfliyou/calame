#![allow(clippy::expect_used)]

use std::io::Cursor;

use calame_core::background::{PAGE_HEIGHT, PAGE_WIDTH};
use calame_core::{
    AssetId, Background, Brush, BrushKind, Color, ImageBox, Item, Page, Point2, Stroke,
    StrokePoint, TextBox,
};
use image::{ImageFormat, RgbaImage};

use super::*;
use crate::pdf::sheets_for;
use crate::raster::parse;

fn stroke(brush: Brush, points: &[(f32, f32)]) -> Item {
    let mut s = Stroke::new(brush);
    for &(x, y) in points {
        s.push(StrokePoint {
            pos: Point2::new(x, y),
            pressure: 1.0,
            tilt: None,
        });
    }
    Item::Stroke(s)
}

fn thick_black(points: &[(f32, f32)]) -> Item {
    stroke(
        Brush {
            kind: BrushKind::Pen,
            color: Color::BLACK,
            width: 12.0,
        },
        points,
    )
}

fn plain(page: Page) -> (Page, PageBackground) {
    (page, PageBackground::Pattern(Background::Blank))
}

fn decode(png: &[u8]) -> RgbaImage {
    image::load_from_memory_with_format(png, ImageFormat::Png)
        .expect("PNG valide")
        .to_rgba8()
}

fn px(img: &RgbaImage, x: f32, y: f32) -> [u8; 3] {
    let p = img.get_pixel(x as u32, y as u32);
    [p[0], p[1], p[2]]
}

fn solid_png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    let img = RgbaImage::from_fn(width, height, |_, _| {
        image::Rgba([rgb[0], rgb[1], rgb[2], 255])
    });
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .expect("encodage");
    out
}

#[test]
fn svg_is_valid_and_describes_every_kind_of_element() {
    let highlighter = Brush {
        kind: BrushKind::Highlighter,
        color: Color {
            r: 255,
            g: 235,
            b: 59,
            a: 110,
        },
        width: 18.0,
    };
    let mut text = TextBox::new(
        Point2::new(50.0, 300.0),
        "A & B <c>\nligne 2".into(),
        20.0,
        Color::rgb(200, 0, 0),
    );
    text.angle = 0.3;
    let page = Page {
        items: vec![
            thick_black(&[(10.0, 10.0), (100.0, 40.0), (200.0, 20.0)]),
            stroke(highlighter, &[(10.0, 100.0), (300.0, 100.0)]),
            thick_black(&[(400.0, 400.0)]),
            Item::Text(text),
        ],
    };
    let svg = page_svg(&page, &PageBackground::Pattern(Background::GridSmall));
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    assert!(svg.contains("<polyline") && svg.contains("stroke-opacity=\"0.43\""));
    assert!(svg.contains("<circle"), "points isolés et bouts ronds");
    assert!(svg.contains("A &amp; B &lt;c&gt;") && svg.contains("ligne 2"));
    assert!(svg.contains("rotate(17.19)"));
    parse(&svg).expect("SVG lisible");
}

#[test]
fn svg_area_grows_with_the_content() {
    let page = Page {
        items: vec![thick_black(&[(100.0, 100.0), (900.0, 2000.0)])],
    };
    let svg = page_svg(&page, &PageBackground::Pattern(Background::Blank));
    let tree = parse(&svg).expect("SVG lisible");
    assert!(tree.size().height() > 2000.0 && tree.size().width() > 900.0);
    let empty = page_svg(
        &Page::default(),
        &PageBackground::Pattern(Background::Blank),
    );
    let tree = parse(&empty).expect("SVG lisible");
    assert!((tree.size().width() - PAGE_WIDTH).abs() < 0.1);
    assert!((tree.size().height() - PAGE_HEIGHT).abs() < 0.1);
}

#[test]
fn png_shows_the_stroke_on_a_white_page() {
    let (page, bg) = plain(Page {
        items: vec![thick_black(&[(100.0, 100.0), (300.0, 100.0)])],
    });
    let png = svg_to_png(&page_svg(&page, &bg), PNG_SCALE).expect("PNG");
    let img = decode(&png);
    assert_eq!(img.width(), (PAGE_WIDTH * PNG_SCALE).ceil() as u32);
    assert_eq!(img.height(), (PAGE_HEIGHT * PNG_SCALE).ceil() as u32);
    let [r, g, b] = px(&img, 200.0 * PNG_SCALE, 100.0 * PNG_SCALE);
    assert!(
        r < 40 && g < 40 && b < 40,
        "trait noir attendu : {r} {g} {b}"
    );
    assert_eq!(px(&img, 10.0, 10.0), [255, 255, 255]);
}

#[test]
fn png_draws_the_page_background_pattern() {
    let blank = svg_to_png(
        &page_svg(
            &Page::default(),
            &PageBackground::Pattern(Background::Blank),
        ),
        1.0,
    )
    .expect("PNG");
    assert!(decode(&blank)
        .pixels()
        .all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255));
    for bg in [
        Background::GridSmall,
        Background::GridLarge,
        Background::Lines,
        Background::Dots,
        Background::Isometric,
    ] {
        let png = svg_to_png(
            &page_svg(&Page::default(), &PageBackground::Pattern(bg)),
            1.0,
        )
        .expect("PNG");
        let marked = decode(&png).pixels().filter(|p| p[0] < 250).count();
        assert!(marked > 300, "{bg:?} : seulement {marked} pixels tracés");
    }
}

#[test]
fn imported_image_is_drawn_behind_the_strokes() {
    let background = PageBackground::Image(BackgroundImage {
        mime: "image/png".into(),
        data: solid_png(4, 4, [220, 20, 20]),
        width: 400.0,
        height: 600.0,
    });
    let page = Page {
        items: vec![thick_black(&[(50.0, 300.0), (350.0, 300.0)])],
    };
    let svg = page_svg(&page, &background);
    let img = decode(&svg_to_png(&svg, 1.0).expect("PNG"));
    assert_eq!((img.width(), img.height()), (400, 600));
    let [r, g, _] = px(&img, 200.0, 100.0);
    assert!(r > 200 && g < 60, "fond rouge attendu : {r} {g}");
    let [r, g, b] = px(&img, 200.0, 300.0);
    assert!(
        r < 40 && g < 40 && b < 40,
        "trait noir par-dessus : {r} {g} {b}"
    );
}

#[test]
fn oversized_exports_are_refused() {
    let page = Page {
        items: vec![thick_black(&[(0.0, 0.0), (30_000.0, 30_000.0)])],
    };
    let svg = page_svg(&page, &PageBackground::Pattern(Background::Blank));
    assert!(matches!(svg_to_png(&svg, 1.0), Err(ExportError::Render(_))));
    assert!(svg_to_png("pas un svg", 1.0).is_err());
}

#[test]
fn placed_images_are_drawn_where_they_sit() {
    let asset = AssetId::new();
    let images = PageImages::from([(
        asset,
        BackgroundImage {
            mime: "image/png".into(),
            data: solid_png(4, 4, [220, 20, 20]),
            width: 100.0,
            height: 50.0,
        },
    )]);
    let page = Page {
        items: vec![Item::Image(ImageBox::new(
            asset,
            Point2::new(200.0, 300.0),
            100.0,
            50.0,
        ))],
    };
    let bg = PageBackground::Pattern(Background::Blank);
    let svg = page_svg_with_images(&page, &bg, &images);
    let img = decode(&svg_to_png(&svg, 1.0).expect("PNG"));
    let [r, g, _] = px(&img, 250.0, 325.0);
    assert!(r > 200 && g < 60, "image rouge attendue : {r} {g}");
    assert_eq!(px(&img, 150.0, 325.0), [255, 255, 255]);

    // Sans son contenu, l'image est omise : la page reste exportable.
    let img = decode(&svg_to_png(&page_svg(&page, &bg), 1.0).expect("PNG"));
    assert_eq!(px(&img, 250.0, 325.0), [255, 255, 255]);
}

#[test]
fn copied_items_are_rendered_alone_on_opaque_white() {
    let items = [thick_black(&[(100.0, 100.0), (300.0, 100.0)])];
    let bitmap = items_to_bitmap(&items, &PageImages::new())
        .expect("rendu")
        .expect("image");
    // Le trait occupe 200 unités sur 12, à 2 pixels par unité, plus les marges : pas la page entière.
    assert!(bitmap.width < 500 && bitmap.height < 100);
    assert_eq!(
        bitmap.rgba.len(),
        (bitmap.width * bitmap.height * 4) as usize
    );
    assert!(bitmap.rgba.chunks_exact(4).all(|p| p[3] == 255));
    assert!(bitmap.rgba.chunks_exact(4).any(|p| p[0] < 40));
    assert!(items_to_bitmap(&[], &PageImages::new())
        .expect("rendu")
        .is_none());
}

#[test]
fn a_page_is_cut_into_as_many_a4_sheets_as_it_needs() {
    // Elle tient sur une feuille : elle garde sa taille.
    let a4 = sheets_for(PAGE_WIDTH, PAGE_HEIGHT);
    assert_eq!((a4.columns, a4.rows, a4.scale), (1, 1, 1.0));
    let small = sheets_for(400.0, 600.0);
    assert_eq!((small.width, small.height, small.rows), (400.0, 600.0, 1));

    // Plus haute qu'une feuille : autant de feuilles A4 qu'il faut, à taille réelle.
    let tall = sheets_for(PAGE_WIDTH, 3000.0);
    assert_eq!((tall.width, tall.height), (PAGE_WIDTH, PAGE_HEIGHT));
    assert_eq!((tall.columns, tall.rows, tall.scale), (1, 3, 1.0));

    // Un peu trop large : réduite à la largeur de la feuille, plutôt que coupée en colonnes.
    let wide = sheets_for(900.0, 2000.0);
    assert_eq!((wide.columns, wide.rows), (1, 2));
    assert!((wide.scale - PAGE_WIDTH / 900.0).abs() < 1e-4);

    // Plus large que haute : A4 paysage.
    let slide = sheets_for(1280.0, 720.0);
    assert_eq!((slide.width, slide.height), (PAGE_HEIGHT, PAGE_WIDTH));
    assert_eq!((slide.columns, slide.rows), (1, 1));

    // Beaucoup trop large pour être réduite : découpée en colonnes, à taille réelle.
    let huge = sheets_for(5000.0, 700.0);
    assert_eq!((huge.columns, huge.rows, huge.scale), (5, 1, 1.0));
}

#[test]
fn a_tall_page_gives_a4_sheets_that_continue_each_other() {
    let tall = Page {
        items: vec![thick_black(&[(100.0, 100.0), (100.0, 3000.0)])],
    };
    let svg = page_svg(&tall, &PageBackground::Pattern(Background::Blank));
    let sheets = render_pdf_pages(svgs_to_pdf(&[svg]).expect("PDF"), 1.0).expect("import");
    assert_eq!(sheets.len(), 3);
    for sheet in &sheets {
        assert!((sheet.width - PAGE_WIDTH).abs() < 2.0, "{}", sheet.width);
        assert!((sheet.height - PAGE_HEIGHT).abs() < 2.0, "{}", sheet.height);
        // Le trait vertical se poursuit d'une feuille à l'autre, à la même abscisse.
        let img = decode(&sheet.png);
        let [r, g, b] = px(&img, 100.0, 500.0);
        assert!(r < 60 && g < 60 && b < 60, "trait attendu : {r} {g} {b}");
        assert_eq!(px(&img, 400.0, 500.0), [255, 255, 255]);
    }
    // Il s'arrête à 3000 unités, soit à 755 unités du haut de la troisième feuille.
    assert_eq!(px(&decode(&sheets[2].png), 100.0, 900.0), [255, 255, 255]);
}

#[test]
fn pdf_keeps_pages_that_fit_and_cuts_the_others() {
    let small = Page::default();
    let tall = Page {
        items: vec![thick_black(&[(0.0, 0.0), (100.0, 3000.0)])],
    };
    let svgs = [
        page_svg(&small, &PageBackground::Pattern(Background::GridLarge)),
        page_svg(&tall, &PageBackground::Pattern(Background::Blank)),
    ];
    let pdf = svgs_to_pdf(&svgs).expect("PDF");
    assert!(pdf.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/Count 4"), "1 feuille, puis 3");
    assert!(svgs_to_pdf(&[]).is_err());
}

#[test]
fn a_pdf_export_can_be_imported_back_and_annotated_over() {
    let page = Page {
        items: vec![thick_black(&[(100.0, 100.0), (300.0, 100.0)])],
    };
    let svg = page_svg(&page, &PageBackground::Pattern(Background::Blank));
    let pdf = svgs_to_pdf(&[svg.clone(), svg]).expect("PDF");

    let pages = render_pdf_pages(pdf, 1.0).expect("import");
    assert_eq!(pages.len(), 2);
    assert!(
        (pages[0].width - PAGE_WIDTH).abs() < 2.0,
        "largeur {}",
        pages[0].width
    );
    assert!(
        (pages[0].height - PAGE_HEIGHT).abs() < 2.0,
        "hauteur {}",
        pages[0].height
    );
    let img = decode(&pages[0].png);
    let [r, g, b] = px(&img, 200.0, 100.0);
    assert!(
        r < 60 && g < 60 && b < 60,
        "trait noir attendu après aller-retour : {r} {g} {b}"
    );
    assert_eq!(px(&img, 20.0, 20.0), [255, 255, 255]);
}

#[test]
fn unreadable_pdfs_and_images_are_rejected_cleanly() {
    assert!(matches!(
        render_pdf_pages(b"ceci n'est pas un PDF".to_vec(), 1.0),
        Err(ExportError::Import(_))
    ));
    assert!(import_image(b"GIF89a....".to_vec()).is_err());
    assert!(import_image(Vec::new()).is_err());
}

#[test]
fn imported_images_keep_their_format_and_fit_the_page_width() {
    let png = solid_png(200, 100, [0, 0, 255]);
    let imported = import_image(png.clone()).expect("PNG");
    assert_eq!(imported.mime, "image/png");
    assert_eq!(imported.data, png);
    assert!((imported.width - PAGE_WIDTH).abs() < 1e-3);
    assert!((imported.height - PAGE_WIDTH / 2.0).abs() < 1e-3);

    let img = RgbaImage::from_fn(30, 60, |_, _| image::Rgba([10, 200, 10, 255]));
    let mut jpeg = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .to_rgb8()
        .write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
        .expect("JPEG");
    let imported = import_image(jpeg).expect("JPEG");
    assert_eq!(imported.mime, "image/jpeg");
    assert!((imported.height - 2.0 * PAGE_WIDTH).abs() < 1e-2);
}

#[test]
fn pasted_images_become_png_and_shrink_when_huge() {
    let small = pasted_image(2, 1, vec![255; 8]).expect("image");
    assert_eq!(small.mime, "image/png");
    assert_eq!((small.width, small.height), (2.0, 1.0));
    assert_eq!(decode(&small.data).width(), 2);

    let huge = pasted_image(5000, 1, vec![255; 20_000]).expect("image");
    assert_eq!(decode(&huge.data).width(), 4096);
    assert_eq!(huge.width, 5000.0);

    assert!(pasted_image(0, 0, Vec::new()).is_err());
    assert!(pasted_image(2, 2, vec![0; 3]).is_err());
}

/// Mesure d'export : `cargo test --release -p calame-export -- --ignored --nocapture`.
#[test]
#[ignore = "mesure de performance"]
fn perf_export_a_dense_page() {
    use std::time::Instant;
    let items: Vec<Item> = (0..1500)
        .map(|i| {
            let (x, y) = ((i % 30) as f32 * 25.0, (i / 30) as f32 * 20.0);
            let pts: Vec<(f32, f32)> = (0..80)
                .map(|j| (x + j as f32 * 0.3, y + 6.0 * (j as f32 * 0.2).sin()))
                .collect();
            thick_black(&pts)
        })
        .collect();
    let page = Page { items };
    let bg = PageBackground::Pattern(Background::GridSmall);
    let start = Instant::now();
    let svg = page_svg(&page, &bg);
    let svg_time = start.elapsed();
    let start = Instant::now();
    let png = svg_to_png(&svg, PNG_SCALE).expect("PNG");
    let png_time = start.elapsed();
    let start = Instant::now();
    let pdf = svgs_to_pdf(std::slice::from_ref(&svg)).expect("PDF");
    let pdf_time = start.elapsed();
    eprintln!(
        "PERF page de 1500 traits : SVG {svg_time:?} ({} Ko), PNG {png_time:?} ({} Ko), PDF {pdf_time:?} ({} Ko)",
        svg.len() / 1000,
        png.len() / 1000,
        pdf.len() / 1000
    );
}

#[test]
fn dense_strokes_do_not_bloat_the_svg() {
    // 5 000 points sur 100 unités : l'œil n'en distingue pas plus de 250.
    let points: Vec<(f32, f32)> = (0..5000)
        .map(|i| (i as f32 * 0.02, 3.0 * (i as f32 * 0.01).sin()))
        .collect();
    let page = Page {
        items: vec![thick_black(&points)],
    };
    let svg = page_svg(&page, &PageBackground::Pattern(Background::Blank));
    assert!(
        svg.len() < 12_000,
        "SVG de {} octets pour un seul trait",
        svg.len()
    );
    parse(&svg).expect("SVG lisible");
}
