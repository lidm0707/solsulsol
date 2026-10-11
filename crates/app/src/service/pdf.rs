//! Billing → grocery-store paper receipt (80 mm roll) with payment QR.
//! Layout: shop header → billing no → items → totals → pay-to → QR → footer.
//! All layout coordinates are top-based mm; converted to PDF's bottom-left
//! origin only inside [`text`] / [`rect`].

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::{BuiltinFont, Color, Image, ImageTransform, Mm, PdfDocument, Rgb};
use rust_decimal::Decimal;

use crate::domain::entities::{Billing, Currency, VatMode};

/// Thermal-roll paper width used by grocery POS printers.
pub const PAPER_W_MM: f32 = 80.0;
pub const MARGIN_MM: f32 = 6.0;
pub const TITLE_SIZE: f32 = 15.0;
pub const BODY_SIZE: f32 = 8.5;
pub const SMALL_SIZE: f32 = 7.0;
/// Vertical advance of one receipt text row.
pub const ROW_MM: f32 = 5.0;
pub const QR_SIZE_MM: f32 = 40.0;
/// Estimated characters per wrapped line of the wallet address.
const ADDR_WRAP_CHARS: usize = 42;

const CONTENT_RIGHT_MM: f32 = PAPER_W_MM - MARGIN_MM;
/// Item / qty / price column left edges (mm from left); amount is right-aligned.
const COLS_MM: [f32; 3] = [6.0, 40.0, 50.0];

// 0..1 rgb — orange & black on paper
const INK: (f32, f32, f32) = (0.08, 0.08, 0.08);
const DIM: (f32, f32, f32) = (0.45, 0.42, 0.38);
const ORANGE: (f32, f32, f32) = (1.0, 0.48, 0.0);
const BAND: (f32, f32, f32) = (1.0, 0.95, 0.9);

#[derive(Debug)]
pub enum PdfError {
    QrDecode(String),
    Save(String),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QrDecode(e) | Self::Save(e) => write!(f, "{e}"),
        }
    }
}

struct Fonts {
    regular: printpdf::IndirectFontRef,
    bold: printpdf::IndirectFontRef,
}

/// Cursor state; all `y` values are top-based mm on paper of [`Ctx::page_h`].
struct Ctx<'a> {
    layer: &'a printpdf::PdfLayerReference,
    fonts: &'a Fonts,
    page_h: f32,
    y: f32,
}

pub fn render_billing_pdf(
    billing: &Billing,
    shop_name: &str,
    qr_png: &[u8],
    currency: Currency,
) -> Result<Vec<u8>, PdfError> {
    let money = MoneyFmt::new(currency);
    let doc = PdfDocument::empty("POS Billing");
    let (page_idx, layer_idx) = doc.add_page(Mm(PAPER_W_MM), Mm(page_height(billing)), "Billing");
    let layer = doc.get_page(page_idx).get_layer(layer_idx);
    let fonts = Fonts {
        regular: doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| PdfError::Save(e.to_string()))?,
        bold: doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| PdfError::Save(e.to_string()))?,
    };
    let qr = image_024::load_from_memory(qr_png).map_err(|e| PdfError::QrDecode(e.to_string()))?;
    let mut c = Ctx {
        layer: &layer,
        fonts: &fonts,
        page_h: page_height(billing),
        y: 0.0,
    };

    header(&mut c, billing, shop_name);
    items(&mut c, billing, &money);
    totals(&mut c, billing, &money);
    qr_card(&mut c, billing, &qr);
    footer(&mut c);

    doc.save_to_bytes()
        .map_err(|e| PdfError::Save(e.to_string()))
}

/// Total paper length for this billing, derived from row counts.
fn page_height(inv: &Billing) -> f32 {
    let vat_rows = if inv.vat_mode == VatMode::None { 0 } else { 2 };
    let addr = inv.destination.address.as_str();
    let addr_rows = addr.len().div_ceil(ADDR_WRAP_CHARS);
    let rows = 4 + vat_rows + inv.lines.len() + addr_rows + 8;
    ROW_MM * rows as f32 + QR_SIZE_MM + 14.0
}

fn header(c: &mut Ctx, billing: &Billing, shop_name: &str) {
    c.y = 8.0;
    text_center(c, shop_name, TITLE_SIZE, c.y, &c.fonts.bold, INK);
    c.y += ROW_MM;
    let id = billing.id.to_string();
    let no = format!("BILLING No. {}", id.get(..8).unwrap_or(&id));
    text_center(c, &no, SMALL_SIZE, c.y, &c.fonts.regular, DIM);
    c.y += 4.0;
    rule(c, c.y, INK);
    c.y += 3.0;
}

/// Amount formatting in the shop pricing currency (line prices are already
/// denominated in it — no conversion).
struct MoneyFmt {
    symbol: &'static str,
}

impl MoneyFmt {
    fn new(currency: Currency) -> Self {
        Self {
            symbol: if currency == Currency::Thb {
                "\u{0e3f}"
            } else {
                "$"
            },
        }
    }

    fn fmt(&self, v: Decimal) -> String {
        format!("{} {}", self.symbol, v.round_dp(2).normalize())
    }
}

fn items(c: &mut Ctx, billing: &Billing, money: &MoneyFmt) {
    text(
        c,
        "ITEM",
        SMALL_SIZE,
        Mm(COLS_MM[0]),
        c.y,
        &c.fonts.bold,
        DIM,
    );
    text(
        c,
        "QTY",
        SMALL_SIZE,
        Mm(COLS_MM[1]),
        c.y,
        &c.fonts.bold,
        DIM,
    );
    text(
        c,
        "PRICE",
        SMALL_SIZE,
        Mm(COLS_MM[2]),
        c.y,
        &c.fonts.bold,
        DIM,
    );
    text_right(
        c,
        "AMOUNT",
        SMALL_SIZE,
        CONTENT_RIGHT_MM,
        c.y,
        &c.fonts.bold,
        DIM,
    );
    c.y += ROW_MM + 1.0;

    for line in &billing.lines {
        text(
            c,
            &line.name,
            BODY_SIZE,
            Mm(COLS_MM[0]),
            c.y,
            &c.fonts.regular,
            INK,
        );
        text(
            c,
            &line.qty.to_string(),
            BODY_SIZE,
            Mm(COLS_MM[1]),
            c.y,
            &c.fonts.regular,
            INK,
        );
        text(
            c,
            &money.fmt(line.price.amount()),
            BODY_SIZE,
            Mm(COLS_MM[2]),
            c.y,
            &c.fonts.regular,
            INK,
        );
        let amount = line.price.amount() * Decimal::from(line.qty);
        text_right(
            c,
            &money.fmt(amount),
            BODY_SIZE,
            CONTENT_RIGHT_MM,
            c.y,
            &c.fonts.bold,
            INK,
        );
        c.y += ROW_MM;
    }
    c.y += 2.0;
    rule(c, c.y, DIM);
    c.y += 3.0;
}

fn totals(c: &mut Ctx, billing: &Billing, money: &MoneyFmt) {
    if billing.vat_mode != VatMode::None {
        summary_row(c, "Subtotal", billing.subtotal(), money);
        summary_row(
            c,
            &format!("VAT {}%", billing.vat_rate.normalize()),
            billing.vat_amount(),
            money,
        );
    }
    rect(
        c,
        MARGIN_MM,
        c.y,
        PAPER_W_MM - MARGIN_MM * 2.0,
        ROW_MM + 1.5,
        BAND,
    );
    rect(c, MARGIN_MM, c.y, 1.4, ROW_MM + 1.5, ORANGE);
    text(
        c,
        "TOTAL",
        12.0,
        Mm(MARGIN_MM + 3.0),
        c.y + 1.8,
        &c.fonts.bold,
        INK,
    );
    text_right(
        c,
        &money.fmt(billing.total()),
        12.0,
        CONTENT_RIGHT_MM - 1.5,
        c.y + 1.8,
        &c.fonts.bold,
        INK,
    );
    c.y += ROW_MM + 1.5;
    summary_row(c, "cost basis", billing.cost(), money);
    c.y += 2.0;
    rule(c, c.y, DIM);
    c.y += 3.0;
}

fn summary_row(c: &mut Ctx, label: &str, amount: Decimal, money: &MoneyFmt) {
    text(
        c,
        label,
        SMALL_SIZE,
        Mm(MARGIN_MM),
        c.y,
        &c.fonts.regular,
        DIM,
    );
    text_right(
        c,
        &money.fmt(amount),
        SMALL_SIZE,
        CONTENT_RIGHT_MM,
        c.y,
        &c.fonts.regular,
        DIM,
    );
    c.y += ROW_MM;
}

fn qr_card(c: &mut Ctx, billing: &Billing, qr: &image_024::DynamicImage) {
    text(
        c,
        "PAY TO",
        SMALL_SIZE,
        Mm(MARGIN_MM),
        c.y,
        &c.fonts.bold,
        DIM,
    );
    text(
        c,
        &format!("({})", billing.destination.address.chain().as_str()),
        SMALL_SIZE,
        Mm(MARGIN_MM + 14.0),
        c.y,
        &c.fonts.regular,
        DIM,
    );
    c.y += ROW_MM;
    wrap_text(
        c,
        billing.destination.address.as_str(),
        BODY_SIZE,
        MARGIN_MM,
        c.y,
        ADDR_WRAP_CHARS,
        INK,
    );
    c.y += ROW_MM * 2.0 + 2.0;

    let qr_x = (PAPER_W_MM - QR_SIZE_MM) / 2.0;
    // printpdf sizes images as px-at-dpi; pick dpi so the QR is exactly QR_SIZE_MM wide.
    let dpi = qr.width() as f32 * 25.4 / QR_SIZE_MM;
    let img = Image::from_dynamic_image(qr);
    img.add_to_layer(
        c.layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(qr_x)),
            translate_y: Some(Mm(c.page_h - c.y - QR_SIZE_MM)),
            dpi: Some(dpi),
            ..ImageTransform::default()
        },
    );
    text_center(
        c,
        "Scan to pay",
        SMALL_SIZE,
        c.y + QR_SIZE_MM + 2.0,
        &c.fonts.bold,
        INK,
    );
    c.y += QR_SIZE_MM + ROW_MM * 2.5;
}

fn footer(c: &mut Ctx) {
    let msg = "Thank you \u{2014} paid on the Solana blockchain";
    text_center(c, msg, SMALL_SIZE, c.page_h - 8.0, &c.fonts.regular, DIM);
}

fn wrap_text(c: &Ctx, s: &str, size: f32, x: f32, y: f32, max_chars: usize, col: (f32, f32, f32)) {
    let mut rest = s;
    let mut dy = 0.0;
    while rest.len() > max_chars {
        let mut cut = max_chars;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        text(c, rest, size, Mm(x), y + dy, &c.fonts.regular, col);
        rest = &rest[cut..];
        dy += ROW_MM;
    }
    text(c, rest, size, Mm(x), y + dy, &c.fonts.regular, col);
}

/// Helvetica average glyph width ~0.5em; mm = chars * size * 0.5 * pt2mm.
fn est_width(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size * 0.5 * 0.3528
}

fn text_center(
    c: &Ctx,
    s: &str,
    size: f32,
    y: f32,
    font: &printpdf::IndirectFontRef,
    col: (f32, f32, f32),
) {
    let x = (PAPER_W_MM - est_width(s, size)) / 2.0;
    text(c, s, size, Mm(x), y, font, col);
}

fn text_right(
    c: &Ctx,
    s: &str,
    size: f32,
    right_x: f32,
    y: f32,
    font: &printpdf::IndirectFontRef,
    col: (f32, f32, f32),
) {
    let x = right_x - est_width(s, size);
    text(c, s, size, Mm(x), y, font, col);
}

/// Horizontal divider line across the printable content.
fn rule(c: &Ctx, y: f32, col: (f32, f32, f32)) {
    rect(c, MARGIN_MM, y, PAPER_W_MM - MARGIN_MM * 2.0, 0.4, col);
}

/// Filled rectangle; `x`/`y` are the top-left corner in top-based mm.
fn rect(c: &Ctx, x: f32, y: f32, w: f32, h: f32, col: (f32, f32, f32)) {
    c.layer
        .set_fill_color(Color::Rgb(Rgb::new(col.0, col.1, col.2, None)));
    let mut r = printpdf::Rect::new(Mm(x), Mm(c.page_h - y - h), Mm(x + w), Mm(c.page_h - y));
    r.mode = PaintMode::Fill;
    r.winding = WindingOrder::NonZero;
    c.layer.add_rect(r);
}

fn text(
    c: &Ctx,
    s: &str,
    size: f32,
    x: Mm,
    y: f32,
    font: &printpdf::IndirectFontRef,
    col: (f32, f32, f32),
) {
    c.layer
        .set_fill_color(Color::Rgb(Rgb::new(col.0, col.1, col.2, None)));
    c.layer
        .use_text(s.to_owned(), size, x, Mm(c.page_h - y), font);
}
