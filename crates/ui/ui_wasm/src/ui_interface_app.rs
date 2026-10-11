//! Dioxus wasm app root: login → shop POS (sell, goods, wallet) with QR pay.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use pos_api::schema::{
    AddDestinationRequest, AddGoodsRequest, AuthNonceRequest, AuthVerifyRequest, BillingPeriodApi,
    Chain, CheckoutRequest, CreatePaymentOrderRequest, CreateShopRequest, Currency,
    PaymentOrderResponse, SplToken, StorageObjectResponse, SubscriptionResponse,
    SubscriptionStatusApi, UpdateGoodsRequest, UpdateShopSettingsRequest, VatMode,
    VerifyPaymentRequest,
};
use reslt_core::prelude::*;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::HtmlVideoElement;

use crate::api_client::FetchClient;
use crate::camera::{Scanner, supported as camera_supported};
use sol_wallet_adaptor::hooks::{UseWallet, use_wallet};
use sol_wallet_adaptor::wallet::WalletKind;
use ui_interface::traits::ApiClient;

const DEFAULT_BASE: &str = "/api";
const BILLING_TITLE: &str = "Billing";
const DETAIL_TITLE: &str = "Sale detail";
const PAY_METHOD_TITLE: &str = "Choose payment";
const QR_TTL_MS: i64 = 5 * 60 * 1000;
const TICK_MS: i32 = 1000;
const URGENCY_MS: i64 = 30_000;
const LOW_STOCK: i64 = 3;
const HELD_BILL_MAX: usize = 8;
const HELD_LABEL_PREFIX: &str = "Bill";
const HELD_ITEMS_SUFFIX: &str = "items";
const HELD_STRIP_LABEL: &str = "held";
const SUB_PRICE_MONTHLY_LABEL: &str = "$1/month";
const SUB_PRICE_YEARLY_LABEL: &str = "$10/year";
const SUB_PLAN_MONTHLY_LABEL: &str = "Monthly $1";
const SUB_PLAN_YEARLY_LABEL: &str = "Yearly $10";
const SUB_ACTIVE_UNTIL_LABEL: &str = "Active until";
const SUB_PACKAGE_LABEL: &str = "1 shop · 30 goods · 10 MB storage";
const SUB_DELETE_POLICY: &str = "Data is deleted 14 days after expiration — renew within the 7-day recovery window to keep your shop.";
const SUB_GRACE_MSG: &str =
    "Subscription expired — renew within 7 days or your shop will be locked";
const SUB_NEAR_EXPIRY_DAYS: i64 = 3;
const SUB_EXPIRING_PREFIX: &str = "Subscription expires in ";
const SUB_EXPIRING_SUFFIX: &str = " day(s) — renew in Settings";
const BANNER_EXPIRING: &str = "banner expiring";
const SUB_LAPSED_MSG: &str =
    "Subscription lapsed — data may have been deleted. Contact support to recover.";
const SUB_LOCKED_MSG: &str = "Shop locked — subscription lapsed. Renew to regain access";
const SUB_GATE_MSG: &str = "Active subscription required — pay below to enter your shop";
const SUB_CONFIRMED_MSG: &str = "Payment confirmed — subscription active";
const SUB_PENDING_MSG: &str = "Waiting for payment…";
const SUB_CONFIRMING_MSG: &str = "Verifying payment…";
const SHOPS_QUOTA_FULL_LABEL: &str = "Shop quota full";
const NO_SHOPS_QUOTA: usize = 0;
const SUB_SIG_PLACEHOLDER: &str = "paste transaction signature after paying";
const SUB_VERIFY_LABEL: &str = "Verify payment";
const SUB_COPY_LABEL: &str = "copy";
const SUB_MERCHANT_LABEL: &str = "Pay this USDC transfer, then verify:";
const GOODS_NAME_REQUIRED_MSG: &str = "name is required";
const GOODS_NUMBER_INVALID_MSG: &str = "price, cost and units must be valid numbers";
const GOODS_EDIT_LABEL: &str = "edit";
const GOODS_DELETE_LABEL: &str = "delete";
const DEST_DELETE_LABEL: &str = "delete";
const STORAGE_TITLE: &str = "Storage";
const STORAGE_REFRESH_LABEL: &str = "Refresh";
const STORAGE_EMPTY_MSG: &str = "no storage objects yet";
const KEY_SHORT_LEN: usize = 8;
const BYTES_PER_KB: f64 = 1024.0;
const BYTE_UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
const HTTP_FORBIDDEN: &str = "http 403";
const TRUNCATE_ADDR: usize = 4;
const BYTES_PER_MB: i64 = 1024 * 1024;
const TOKEN_AMOUNT_DP: u32 = 5;

/// API base: `?api=<url>` query param overrides the default (used by e2e).
fn api_base() -> String {
    let Some(q) = web_sys::window().and_then(|w| w.location().search().ok()) else {
        return String::from(DEFAULT_BASE);
    };
    q.strip_prefix('?')
        .and_then(|q| {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k != "api" {
                    return None;
                }
                let decoded = js_sys::decode_uri_component(v).ok()?;
                Some(String::from(decoded))
            })
        })
        .unwrap_or_else(|| String::from(DEFAULT_BASE))
}

fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

fn qr_svg(payload: &str) -> Option<String> {
    let code = qrcode::QrCode::new(payload.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color>()
            .quiet_zone(true)
            .build(),
    )
}

fn fmt_remaining(ms_left: i64) -> String {
    let secs = (ms_left / 1000).max(0);
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Sell,
    Goods,
    Reports,
    Settings,
}

impl Tab {
    fn id(self) -> &'static str {
        match self {
            Tab::Sell => "tab-sell",
            Tab::Goods => "tab-goods",
            Tab::Reports => "tab-reports",
            Tab::Settings => "tab-settings",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Tab::Sell => ICON_SELL,
            Tab::Goods => ICON_GOODS,
            Tab::Reports => ICON_REPORTS,
            Tab::Settings => ICON_SETTINGS,
        }
    }
}

const ICON_HOLD: &str = "M8 5h4v14H8zM14 5h4v14h-4z";
const ICON_SELL: &str = "M3 9h18l-1.5 12h-15L3 9Zm4.5 0V6a4.5 4.5 0 0 1 9 0v3";
const ICON_GOODS: &str = "M4 7h16v14H4V7Zm3 0V5a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v2M8 12h8";
const ICON_REPORTS: &str = "M4 20V10m6 10V4m6 16v-7m4 7H2";
const ICON_SETTINGS: &str = "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm7.4-3a7.4 7.4 0 0 0-.1-1.2l2-1.5-2-3.5-2.4 1a7.4 7.4 0 0 0-2-1.2L14.5 3h-5l-.4 2.6a7.4 7.4 0 0 0-2 1.2l-2.4-1-2 3.5 2 1.5a7.4 7.4 0 0 0 0 2.4l-2 1.5 2 3.5 2.4-1a7.4 7.4 0 0 0 2 1.2l.4 2.6h5l.4-2.6a7.4 7.4 0 0 0 2-1.2l2.4 1 2-3.5-2-1.5c.1-.4.1-.8.1-1.2Z";
const ICON_LOGOUT: &str = "M15 4h4v16h-4M11 8l4 4-4 4m4-4H3";
const ICON_ADD: &str = "M12 5v14M5 12h14";
const ICON_QTY: &str = "M4 7h9m4 0h3M4 17h3m4 0h9M15 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5ZM9 14.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5Z";
const ICON_CART: &str =
    "M4 5h2l2.2 11h10.3l2-8H7M10 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2Zm8 0a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z";

const DEFAULT_ICON_KEY: &str = "package";
const DEFAULT_GOODS_TYPE: &str = "other";
const GOODS_TYPE_CHOICES: &[(&str, &str)] = &[
    ("service", "Service"),
    ("food", "Food"),
    ("drink", "Drink"),
    ("dessert", "Dessert"),
    ("electronic", "Electronic"),
    ("clothing", "Clothing"),
    ("cosmetic", "Cosmetic"),
    ("ticket", "Ticket"),
    ("prop", "Cosplay prop"),
    ("other", "Other"),
];

fn goods_type_label(key: &str) -> &'static str {
    GOODS_TYPE_CHOICES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, label)| *label)
        .unwrap_or("Other")
}
const DEFAULT_ICON_PATH: &str = "M12 3 4 7v10l8 4 8-4V7l-8-4ZM4 7l8 4 8-4M12 11v10M8 5l8 4";
const DEFAULT_COLOR_KEY: &str = "c01";
const DEFAULT_COLOR_HEX: &str = "#FFADAD";
const GOODS_SORT_KEY: &str = "pos.goods.sort";
const AUTH_TOKEN_KEY: &str = "pos.auth.token";
const TABLE_SORT_KEY: &str = "pos.goods.table.sort";
const SORT_STORE_SEP: &str = ":";

const ICON_KEYS: &[&str] = &[
    "package",
    "cart",
    "coffee",
    "burger",
    "pizza",
    "soda",
    "water",
    "beer",
    "bread",
    "cake",
    "rice",
    "noodle",
    "apple",
    "carrot",
    "meat",
    "fish",
    "milk",
    "egg",
    "candy",
    "icecream",
    "shirt",
    "hat",
    "shoe",
    "bag",
    "gift",
    "phone",
    "laptop",
    "headphone",
    "camera",
    "book",
    "pen",
    "flower",
    "dog",
    "cat",
    "pill",
    "soap",
    "gas",
    "ticket",
    "wrench",
    "star",
];

const GOODS_ICON_CHOICES: &[(&str, &str)] = &[
    ("package", "Package"),
    ("cart", "Cart"),
    ("coffee", "Coffee"),
    ("burger", "Burger"),
    ("pizza", "Pizza"),
    ("soda", "Soda"),
    ("water", "Water"),
    ("beer", "Beer"),
    ("bread", "Bread"),
    ("cake", "Cake"),
    ("rice", "Rice"),
    ("noodle", "Noodle"),
    ("apple", "Apple"),
    ("carrot", "Carrot"),
    ("meat", "Meat"),
    ("fish", "Fish"),
    ("milk", "Milk"),
    ("egg", "Egg"),
    ("candy", "Candy"),
    ("icecream", "Ice cream"),
    ("shirt", "Shirt"),
    ("hat", "Hat"),
    ("shoe", "Shoe"),
    ("bag", "Bag"),
    ("gift", "Gift"),
    ("phone", "Phone"),
    ("laptop", "Laptop"),
    ("headphone", "Headphone"),
    ("camera", "Camera"),
    ("book", "Book"),
    ("pen", "Pen"),
    ("flower", "Flower"),
    ("dog", "Dog"),
    ("cat", "Cat"),
    ("pill", "Pill"),
    ("soap", "Soap"),
    ("gas", "Gas"),
    ("ticket", "Ticket"),
    ("wrench", "Wrench"),
    ("star", "Star"),
];

const GOODS_COLOR_CHOICES: &[(&str, &str)] = &[
    ("c01", "Rose"),
    ("c02", "Peach"),
    ("c03", "Lemon"),
    ("c04", "Lime"),
    ("c05", "Aqua"),
    ("c06", "Sky"),
    ("c07", "Lavender"),
    ("c08", "Orchid"),
    ("c09", "Shell"),
    ("c10", "Cream"),
    ("c11", "Apricot"),
    ("c12", "Coral"),
    ("c13", "Snow"),
    ("c14", "Blush"),
    ("c15", "Linen"),
    ("c16", "Sage"),
    ("c17", "Fog"),
    ("c18", "Bone"),
    ("c19", "Fern"),
    ("c20", "Olive"),
    ("c21", "Straw"),
    ("c22", "Aloe"),
    ("c23", "Petal"),
    ("c24", "Bubblegum"),
    ("c25", "Salmon"),
    ("c26", "Sand"),
    ("c27", "Celery"),
    ("c28", "Seafoam"),
    ("c29", "Periwinkle"),
    ("c30", "Rosewater"),
    ("c31", "Spearmint"),
    ("c32", "Baby blue"),
    ("c33", "Lilac"),
    ("c34", "Melon"),
    ("c35", "Butter"),
    ("c36", "Powder"),
    ("c37", "Iris"),
    ("c38", "Honey"),
    ("c39", "Moss"),
    ("c40", "Flamingo"),
];

const ICON_PATHS: &[&str] = &[
    "M12 3 4 7v10l8 4 8-4V7l-8-4ZM4 7l8 4 8-4M12 11v10M8 5l8 4",
    "M4 5h2l2.4 11h10.2L21 8H7M10 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2Zm8 0a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z",
    "M4 8h12v6a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V8Zm12 2h2a2.5 2.5 0 0 1 0 5h-2M7 4v2m3-3v3m3-2v2",
    "M4 12h16l-1 8H5l-1-8Zm4 0V9a4 4 0 0 1 8 0v3M8 15h8",
    "M12 3l8 5-2 12H6L4 8l8-5Zm0 0v17M4 8l8 3 8-3M9 20l1-5m5 5-1-5",
    "M8 3h8l-1 4H9L8 3Zm1 4h6v12a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2V7Zm2 6h2",
    "M12 3c3 4 5 6.5 5 9.5A5 5 0 0 1 7 12.5C7 9.5 9 7 12 3Zm-2 10a2 2 0 0 0 2 2",
    "M6 4h12v6a6 6 0 0 1-12 0V4Zm0 2H4a2 2 0 0 0 2 4m12-4h2a2 2 0 0 1-2 4m-6 4v3m-3 4h6m-5 0 1-3h2l1 3",
    "M4 12c0-4 3-7 8-7s8 3 8 7-3 7-8 7-8-3-8-7Zm3 0h10M9 9h6",
    "M5 13c0-4 3-7 7-7s7 3 7 7v2H5v-2Zm-1 4h16v3H4v-3Zm8-11V4",
    "M8 4h8v14a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2V4Zm0 3h8M8 12h8",
    "M12 3l2 4h5l-4 5 2 8-5-4-5 4 2-8-4-5h5l2-4Z",
    "M12 8c4 0 6 2.5 6 6s-2 7-6 7-6-3.5-6-7 2-6 6-6Zm0 0C12 5 14 3 16 3m-6 9v4",
    "M3 12c3-5 6-6 9-6s6 1 9 6c-3 5-6 6-9 6s-6-1-9-6Zm9-2a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z",
    "M6 4h12v16H6V4Zm3 3h6m-6 3h6m-6 3h4",
    "M5 12c0-4 3-7 7-7s7 3 7 7-3 7-7 7-7-3-7-7Zm2-1h10",
    "M5 4h14v14H5V4Zm3 4h8m-8 3h8m-8 3h5",
    "M4 7h16v13H4V7Zm5 0V5a3 3 0 0 1 6 0v2",
    "M4 8c4-5 12-5 16 0-4 5-12 5-16 0Zm8 0v8m-4-2c1 3 7 3 8 0",
    "M4 6h16v10H10l-4 4v-4H4V6Zm4 3h8m-8 3h5",
    "M4 9h16v10H4V9Zm2-2 2-3h8l2 3",
    "M9 3h6v4a3 3 0 0 1-6 0V3Zm0 2H6m12 0h-3M12 10v11m-3 0h6",
    "M5 7h14v13H5V7Zm5 0V5a2 2 0 0 1 4 0v2M9 12h6",
    "M6 3h12v18H6V3Zm3 0v4h6V3M9 12h6",
    "M7 3h10v18l-5-3-5 3V3Zm3 5h4",
    "M4 8h4l2-3h4l2 3h4v11H4V8Zm8 3a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z",
    "M7 4h7l4 4v13H7V4Zm7 0v4h4",
    "M12 3v18M8 7c0-2 8-2 8 0s-8 2-8 4 8 2 8 4-8 2-8 0",
    "M4 12a8 8 0 0 1 16 0M2 12h3m14 0h3M8 12a4 4 0 0 1 8 0m-11 4h14v5H3v-5Z",
    "M5 4h14v16l-3-2-2 2-2-2-2 2-2-2-3 2V4Zm4 5h6m-6 4h6",
    "M5 4h10l4 4v12H5V4Zm10 0v4h4",
    "M8 4h8v6a4 4 0 0 1-8 0V4Zm4 10v6m-3 0h6m-5-9a2 2 0 0 0 4 0",
    "M3 10c4-6 14-6 18 0-4 6-14 6-18 0Zm9 1a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z",
    "M6 9V5a3 3 0 0 1 6 0v4m3 0h4v11H3V9h12Z",
    "M7 4h10v16l-5-4-5 4V4Zm3 4h4m-4 3h4",
    "M4 6h16v13H4V6Zm4-2h8v2H8V4Zm5 6h5m-5 3h5m-8-3h.01M9 13h.01",
    "M12 5c4 0 7 2 7 5 0 2-2 3-3 3l1 6h-4l-.5-4h-1L11 19H7l1-6c-1 0-3-1-3-3 0-3 3-5 7-5Z",
    "M4 19c2-8 6-12 16-14-1 10-6 14-12 14H4Zm4 0c1-4 3-7 7-9",
    "M4 4h16v14H4V4Zm8 4 1.2 2.4 2.8.4-2 1.9.5 2.6L12 14l-2.5 1.3.5-2.6-2-1.9 2.8-.4L12 8Z",
];

const GOODS_COLORS: &[(&str, &str)] = &[
    ("c01", "#FFADAD"),
    ("c02", "#FFD6A5"),
    ("c03", "#FDFFB6"),
    ("c04", "#CAFFBF"),
    ("c05", "#9BF6FF"),
    ("c06", "#A0C4FF"),
    ("c07", "#BDB2FF"),
    ("c08", "#FFC6FF"),
    ("c09", "#FDE4CF"),
    ("c10", "#FBF8CC"),
    ("c11", "#FFC9B9"),
    ("c12", "#FFB5A7"),
    ("c13", "#F8EDEB"),
    ("c14", "#FCD5CE"),
    ("c15", "#FAE1DD"),
    ("c16", "#D8E2DC"),
    ("c17", "#E8E8E4"),
    ("c18", "#ECEAE4"),
    ("c19", "#CFE1B9"),
    ("c20", "#E9EDC9"),
    ("c21", "#CCD5AE"),
    ("c22", "#E9F5DB"),
    ("c23", "#FFE5EC"),
    ("c24", "#FFC8DD"),
    ("c25", "#FFB7B2"),
    ("c26", "#FFDAC1"),
    ("c27", "#E2F0CB"),
    ("c28", "#B5EAD7"),
    ("c29", "#C7CEEA"),
    ("c30", "#F7D6E0"),
    ("c31", "#D0F4DE"),
    ("c32", "#A9DEF9"),
    ("c33", "#E4C1F9"),
    ("c34", "#FFD9C0"),
    ("c35", "#FFF3B0"),
    ("c36", "#D6E6F2"),
    ("c37", "#EDE7FF"),
    ("c38", "#FFE0AC"),
    ("c39", "#C9E4C5"),
    ("c40", "#F6C6EA"),
];

fn icon_path(key: &str) -> &'static str {
    ICON_KEYS
        .iter()
        .position(|k| *k == key)
        .and_then(|i| ICON_PATHS.get(i))
        .copied()
        .unwrap_or(DEFAULT_ICON_PATH)
}

fn color_hex(key: &str) -> &'static str {
    GOODS_COLORS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, hex)| *hex)
        .unwrap_or(DEFAULT_COLOR_HEX)
}

fn ls_get(key: &str) -> Option<String> {
    let store = web_sys::window()?.local_storage().ok()??;
    store.get_item(key).ok()?
}

fn ls_set(key: &str, value: &str) {
    if let Some(Some(store)) = web_sys::window().map(|w| w.local_storage().ok().flatten()) {
        let _ = store.set_item(key, value);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum GoodsSort {
    Best,
    Price,
    Name,
}

impl GoodsSort {
    const ALL: [Self; 3] = [Self::Best, Self::Price, Self::Name];
    const BEST_LABEL: &str = "Best-selling";
    const PRICE_LABEL: &str = "Price";
    const NAME_LABEL: &str = "Name";

    fn label(self) -> &'static str {
        match self {
            Self::Best => Self::BEST_LABEL,
            Self::Price => Self::PRICE_LABEL,
            Self::Name => Self::NAME_LABEL,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Price => "price",
            Self::Name => "name",
        }
    }

    fn parse(key: &str) -> Self {
        match key {
            "price" => Self::Price,
            "name" => Self::Name,
            _ => Self::Best,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SaleView {
    New,
    Docs,
}

impl SaleView {
    const NEW_LABEL: &str = "New sale";
    const DOCS_LABEL: &str = "Documents";
}

const SCAN_PLACEHOLDER: &str = "scan barcode or search name, Enter to add";
const CART_TITLE: &str = "Cart";
const CART_CLEAR_LABEL: &str = "clear";
const CART_ITEMS_WORD: &str = "items";
const CART_EMPTY_MSG: &str = "cart is empty — tap goods to add";
const QTY_DONE_LABEL: &str = "done";
const SEARCH_NO_MATCH_MSG: &str = "no matching goods: ";
const SEARCH_MULTI_MATCH_MSG: &str = "multiple goods match, type more: ";
const UNKNOWN_BARCODE_PREFIX: &str = "unknown barcode: ";
const CAMERA_START_LABEL: &str = "Scan with camera";
const CAMERA_STOP_LABEL: &str = "Stop camera";
const CAMERA_UNSUPPORTED_MSG: &str = "camera scanning not supported in this browser";
const CAMERA_ERROR_MSG: &str = "could not open camera — check permission";
const CAMERA_VIDEO_ID: &str = "scan-video";
const SCAN_INTERVAL_MS: u32 = 250;
const MOUNT_RETRY_MS: u32 = 100;

const BARCODE_VIDEO_ID: &str = "barcode-video";
const BARCODE_FMT_AUTO: &str = "auto";
const BARCODE_AUTO_LABEL: &str = "Auto-detect";
const BARCODE_SCAN_START_LABEL: &str = "Scan";
const BARCODE_OK_PREFIX: &str = "\u{2713} valid: ";
const FIELD_LABEL_PRICE: &str = "Price";
const FIELD_LABEL_COST: &str = "Cost";
const FIELD_LABEL_UNITS: &str = "Units";
const FIELD_LABEL_ICON: &str = "Icon";
const FIELD_LABEL_COLOR: &str = "Color";
const FIELD_LABEL_BARCODE: &str = "Barcode (scan or type)";
const UNITS_HINT: &str = "units 0 = stock not tracked";
const BARCODE_FORMAT_CHOICES: &[(&str, &str)] = &[
    ("auto", BARCODE_AUTO_LABEL),
    ("ean13", "EAN-13"),
    ("ean8", "EAN-8"),
    ("upca", "UPC-A"),
    ("code39", "Code39"),
];

fn barcode_format_parse(key: &str) -> Option<pos_barcode::format::BarcodeFormat> {
    use pos_barcode::format::BarcodeFormat;
    match key {
        "ean13" => Some(BarcodeFormat::Ean13),
        "ean8" => Some(BarcodeFormat::Ean8),
        "upca" => Some(BarcodeFormat::UpcA),
        "code39" => Some(BarcodeFormat::Code39),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CartItem {
    goods_id: Uuid,
    qty: u32,
}

const CART_QTY_STEP: u32 = 1;

impl CartItem {
    fn new(goods_id: Uuid) -> Self {
        Self {
            goods_id,
            qty: CART_QTY_STEP,
        }
    }

    fn bump(&mut self, by: u32) {
        self.qty += by;
    }
}

fn add_by_barcode(
    goods: &[pos_api::schema::GoodsResponse],
    code: &str,
    selected: &mut Vec<CartItem>,
) -> bool {
    match goods.iter().find(|g| g.barcode.as_deref() == Some(code)) {
        Some(g) => {
            match selected.iter_mut().find(|c| c.goods_id == g.goods_id) {
                Some(item) => item.bump(CART_QTY_STEP),
                None => selected.push(CartItem::new(g.goods_id)),
            }
            true
        }
        None => false,
    }
}

async fn run_camera_scanner(
    mut camera_on: Signal<bool>,
    goods: Signal<Vec<pos_api::schema::GoodsResponse>>,
    mut selected: Signal<Vec<CartItem>>,
    mut toast: UseToast,
) {
    let video = loop {
        match web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(CAMERA_VIDEO_ID))
            .and_then(|e| e.dyn_into::<HtmlVideoElement>().ok())
        {
            Some(v) => break v,
            None => TimeoutFuture::new(MOUNT_RETRY_MS).await,
        }
    };
    let scanner = match Scanner::start(&video).await {
        Ok(s) => s,
        Err(_) => {
            camera_on.set(false);
            notify(&mut toast, CAMERA_ERROR_MSG, true);
            return;
        }
    };
    while *camera_on.peek() {
        TimeoutFuture::new(SCAN_INTERVAL_MS).await;
        let Some(raw) = scanner.read_once(&video).await else {
            continue;
        };
        let code = pos_barcode::format::normalize(&raw);
        if code.is_empty() {
            continue;
        }
        let mut cur = selected.peek().clone();
        if add_by_barcode(&goods.peek()[..], &code, &mut cur) {
            selected.set(cur);
        } else {
            notify(&mut toast, &format!("{UNKNOWN_BARCODE_PREFIX}{code}"), true);
        }
        break;
    }
    scanner.stop();
    camera_on.set(false);
}

async fn run_barcode_capture(
    mut camera_on: Signal<bool>,
    mut barcode: Signal<String>,
    mut toast: UseToast,
) {
    let video = loop {
        match web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(BARCODE_VIDEO_ID))
            .and_then(|e| e.dyn_into::<HtmlVideoElement>().ok())
        {
            Some(v) => break v,
            None => TimeoutFuture::new(MOUNT_RETRY_MS).await,
        }
    };
    let scanner = match Scanner::start(&video).await {
        Ok(s) => s,
        Err(_) => {
            camera_on.set(false);
            notify(&mut toast, CAMERA_ERROR_MSG, true);
            return;
        }
    };
    while *camera_on.peek() {
        TimeoutFuture::new(SCAN_INTERVAL_MS).await;
        let Some(raw) = scanner.read_once(&video).await else {
            continue;
        };
        let code = pos_barcode::format::normalize(&raw);
        if code.is_empty() {
            continue;
        }
        barcode.set(code);
        break;
    }
    scanner.stop();
    camera_on.set(false);
}

#[derive(Clone, Copy, PartialEq)]
enum PayState {
    Idle,
    Pending,
    Confirming,
    Confirmed,
    Failed,
}

#[derive(Clone, Copy, PartialEq)]
enum SettingsGroup {
    Subscription,
    Payments,
    Usage,
}

impl SettingsGroup {
    fn id(self) -> &'static str {
        match self {
            Self::Subscription => "group-subscription",
            Self::Payments => "group-payments",
            Self::Usage => "group-usage",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Subscription => "Subscription",
            Self::Payments => "Payments",
            Self::Usage => "Usage",
        }
    }
    fn default_section(self) -> SettingsSection {
        match self {
            Self::Subscription => SettingsSection::Renew,
            Self::Payments => SettingsSection::Receive,
            Self::Usage => SettingsSection::Usage,
        }
    }
    fn sections(self) -> &'static [SettingsSection] {
        match self {
            Self::Subscription => &[SettingsSection::Renew],
            Self::Payments => &[
                SettingsSection::Receive,
                SettingsSection::Tax,
                SettingsSection::Currency,
            ],
            Self::Usage => &[SettingsSection::Usage, SettingsSection::Storage],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SettingsSection {
    Renew,
    Receive,
    AddAccount,
    Tax,
    Currency,
    Usage,
    Storage,
}

impl SettingsSection {
    fn id(self) -> &'static str {
        match self {
            Self::Renew => "section-renew",
            Self::Receive => "section-receive",
            Self::AddAccount => "section-add-account",
            Self::Tax => "section-tax",
            Self::Currency => "section-currency",
            Self::Usage => "section-usage",
            Self::Storage => "section-storage",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Renew => "Renew / subscribe",
            Self::Receive => "Receive accounts",
            Self::AddAccount => "Add account",
            Self::Tax => "Tax / VAT",
            Self::Currency => "Currency & FX",
            Self::Usage => "Usage",
            Self::Storage => "Storage",
        }
    }
}

#[component]
fn Icon(path: &'static str, size: u32) -> Element {
    rsx! {
        svg {
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            "stroke-width": "1.8",
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            path { d: "{path}" }
        }
    }
}

#[component]
fn GoodsIcon(icon: String, color: String, size: u32) -> Element {
    let path = icon_path(&icon);
    let hex = color_hex(&color);
    rsx! {
        span { class: "goods-img", style: "background:{hex};color:#221418",
            Icon { path, size }
        }
    }
}

fn goods_icon_of(g: &pos_api::schema::GoodsResponse) -> Element {
    let icon = if g.icon.is_empty() {
        String::from(DEFAULT_ICON_KEY)
    } else {
        g.icon.clone()
    };
    let color = if g.color.is_empty() {
        String::from(DEFAULT_COLOR_KEY)
    } else {
        g.color.clone()
    };
    rsx! { GoodsIcon { icon, color, size: 24 } }
}

pub fn app_root() -> Element {
    let token = use_signal(|| ls_get(AUTH_TOKEN_KEY).filter(|t| !t.is_empty()));
    let base = use_signal(api_base);
    use_toast_provider();

    let check_base = base();
    use_future(move || {
        let client = FetchClient {
            base_url: check_base.clone(),
        };
        let mut token = token;
        async move {
            let Some(t) = token() else { return };
            if client.auth_me(&t).await.is_err() {
                ls_set(AUTH_TOKEN_KEY, "");
                token.set(None);
            }
        }
    });

    rsx! {
        style { {STYLE} }
        div { class: "app",
            if token().is_some() {
                Dashboard { token, base: base() }
            } else {
                AuthView { token, base: base() }
            }
            ToastHost {}
        }
    }
}

const TOAST_TTL_MS: i32 = 4_000;
const MAX_TOASTS: usize = 3;

/// Show a toast and schedule its auto-dismiss.
fn notify(toast: &mut UseToast, message: &str, is_error: bool) {
    if toast.get_toasts().iter().any(|t| t.message == message) {
        return;
    }
    if is_error {
        toast.error(message);
    } else {
        toast.success(message);
    }
    let id = toast.counter.peek().to_owned();
    let mut toast = toast.clone();
    toast_cap(&mut toast);
    spawn(async move {
        TimeoutFuture::new(TOAST_TTL_MS as u32).await;
        toast.remove(id);
    });
}

fn toast_cap(toast: &mut UseToast) {
    let mut extra = toast.get_toasts().len().saturating_sub(MAX_TOASTS);
    while extra > 0 {
        let oldest = toast.get_toasts().first().map(|t| t.id);
        match oldest {
            Some(id) => toast.remove(id),
            None => break,
        }
        extra -= 1;
    }
}

#[component]
fn ToastHost() -> Element {
    let toast = use_toast();
    rsx! {
        div { class: "toast-host",
            for t in toast.get_toasts() {
                ToastItem { toast: t }
            }
        }
    }
}

#[component]
fn ToastItem(toast: Toast) -> Element {
    let mut toast_handle = use_toast();
    let id = toast.id;
    rsx! {
        div { class: "toast {toast_class(&toast.toast_type)}",
            key: "{toast.id}",
            span { "{toast.message}" }
            button {
                class: "toast-close",
                "aria-label": "close-toast",
                onclick: move |_| toast_handle.remove(id),
                "\u{2715}"
            }
        }
    }
}

fn toast_class(kind: &ToastType) -> &'static str {
    match kind {
        ToastType::Success => "success",
        ToastType::Error => "error",
        ToastType::Warning => "warning",
        ToastType::Info => "info",
    }
}

#[component]
fn AuthView(token: Signal<Option<String>>, base: String) -> Element {
    let wallet = use_wallet();
    let busy = use_signal(|| false);
    let toast = use_toast();

    let on_connect = move |kind: WalletKind| {
        let mut w = UseWallet {
            account: wallet.account,
            busy: wallet.busy,
            error: wallet.error,
        };
        spawn(async move {
            w.connect(kind).await;
        });
    };
    let on_disconnect = {
        let mut wallet = wallet.clone();
        move |_| {
            wallet.disconnect();
            ls_set(AUTH_TOKEN_KEY, "");
            token.set(None);
        }
    };
    let on_sign_in = {
        let wallet = wallet.clone();
        move |_| {
            let client = FetchClient {
                base_url: base.clone(),
            };
            let mut busy = busy;
            let mut toast = toast.clone();
            let mut wallet = wallet.clone();
            let mut token = token;
            spawn(async move {
                busy.set(true);
                let Some(account) = wallet.account.read().clone() else {
                    busy.set(false);
                    return;
                };
                let nonce = match client
                    .auth_nonce(&AuthNonceRequest {
                        wallet: account.pubkey.clone(),
                    })
                    .await
                {
                    Ok(n) => n,
                    Err(e) => {
                        notify(&mut toast, &e, true);
                        busy.set(false);
                        return;
                    }
                };
                let Some(signature) = wallet.sign_message(&nonce.message).await else {
                    if let Some(e) = wallet.error.read().clone() {
                        notify(&mut toast, &e, true);
                    }
                    busy.set(false);
                    return;
                };
                match client
                    .auth_verify(&AuthVerifyRequest {
                        wallet: account.pubkey,
                        nonce: nonce.nonce,
                        signature,
                    })
                    .await
                {
                    Ok(res) => {
                        ls_set(AUTH_TOKEN_KEY, &res.token);
                        token.set(Some(res.token));
                    }
                    Err(e) => notify(&mut toast, &e, true),
                }
                busy.set(false);
            });
        }
    };

    rsx! {
        div { class: "auth",
            div { class: "brand",
                h1 { "solsulsol" }
                p { class: "dim", "tap goods · charge · get paid" }
            }
            div { class: "card form",
                match wallet.account.read().clone() {
                    Some(account) => rsx! {
                        div { class: "row center",
                            span { class: "chip mono", "{truncate(&account.pubkey, TRUNCATE_ADDR)}" }
                            span { class: "dim small", "{account.kind.name()}" }
                        }
                        button {
                            class: "btn primary full",
                            disabled: busy() || *wallet.busy.read(),
                            onclick: on_sign_in,
                            if busy() { "…" } else { "Sign In" }
                        }
                        button {
                            class: "btn ghost full",
                            "aria-label": "disconnect-wallet",
                            onclick: on_disconnect,
                            "Disconnect"
                        }
                    },
                    None => rsx! {
                        p { class: "dim small", "Connect a Solana wallet to sign in" }
                        for kind in WalletKind::ALL {
                            button {
                                class: "btn primary full",
                                disabled: "{wallet.busy}",
                                "aria-label": "connect-wallet",
                                onclick: move |_| on_connect(kind),
                                "{wallet_label(wallet.busy, kind.name())}"
                            }
                        }
                        if let Some(e) = wallet.error.read().clone() {
                            div { class: "error", "{e}" }
                        }
                    },
                }
            }
        }
    }
}

const SUB_GATE_TITLE: &str = "Subscription required";

/// Enters the shop only when the fetched subscription is Active/Grace (and
/// near expiry still warns). Unpaid → payment gate, no entry. Returns whether
/// entry is allowed.
async fn sub_gate_enter(
    base: String,
    token: Signal<Option<String>>,
    mut sub: Signal<Option<SubscriptionResponse>>,
    mut toast: UseToast,
    mut pay_gate: Signal<bool>,
) -> bool {
    let client = FetchClient { base_url: base };
    match client
        .subscription_current(&token().unwrap_or_default())
        .await
    {
        Ok(s)
            if matches!(
                s.status,
                SubscriptionStatusApi::Active | SubscriptionStatusApi::Grace
            ) =>
        {
            sub.set(Some(s));
            true
        }
        Ok(s) => {
            let locked = matches!(
                s.status,
                SubscriptionStatusApi::Lapsed | SubscriptionStatusApi::Deleted
            );
            sub.set(Some(s));
            pay_gate.set(true);
            notify(
                &mut toast,
                if locked { SUB_LOCKED_MSG } else { SUB_GATE_MSG },
                true,
            );
            false
        }
        Err(_) => {
            pay_gate.set(true);
            notify(&mut toast, SUB_GATE_MSG, true);
            false
        }
    }
}

#[component]
fn Dashboard(token: Signal<Option<String>>, base: String) -> Element {
    use_modal_provider();
    let token_str = move || token().unwrap_or_default();
    let mut shop_id = use_signal(|| query_param("shop").and_then(|v| v.parse::<Uuid>().ok()));
    let shops = use_signal(Vec::<pos_api::schema::ShopResponse>::new);
    let mut new_shop = use_signal(String::new);
    let mut sub = use_signal(|| Option::<SubscriptionResponse>::None);
    let mut gate_shop = use_signal(|| Option::<Uuid>::None);
    use_effect(move || match shop_id() {
        Some(sid) => push_query_param("shop", &sid.to_string()),
        None => push_query_param("shop", ""),
    });
    let modal = use_modal();
    let toast = use_toast();

    let base_future = base.clone();
    let sub_future = base.clone();
    let fut_toast = toast.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: base_future.clone(),
        };
        let mut shops = shops;
        let mut toast = fut_toast.clone();
        let token = token;
        async move {
            match client.list_shops(&token().unwrap_or_default()).await {
                Ok(s) => shops.set(s),
                Err(e) => notify(&mut toast, &e, true),
            }
        }
    });
    use_future(move || {
        let client = FetchClient {
            base_url: sub_future.clone(),
        };
        let mut sub = sub;
        async move {
            if let Ok(s) = client
                .subscription_current(&token().unwrap_or_default())
                .await
            {
                sub.set(Some(s));
            }
        }
    });

    let sub_is_active = move || {
        matches!(
            sub().map(|s| s.status),
            Some(SubscriptionStatusApi::Active) | Some(SubscriptionStatusApi::Grace)
        )
    };
    let quota_full = move || {
        let quota = sub().map(|s| s.shops_quota).unwrap_or(NO_SHOPS_QUOTA);
        quota > NO_SHOPS_QUOTA
            && (shops().len() >= quota || sub().is_some_and(|s| s.shops_used >= s.shops_quota))
    };
    let gate_base = base.clone();
    let gate_toast = toast.clone();
    let mut pay_gate = use_signal(|| false);
    let effect_base = gate_base.clone();
    let effect_toast = gate_toast.clone();
    use_effect(move || {
        let Some(sid) = gate_shop() else { return };
        let enter_base = effect_base.clone();
        let enter_toast = effect_toast.clone();
        spawn(async move {
            if sub_gate_enter(enter_base, token, sub, enter_toast, pay_gate).await {
                shop_id.set(Some(sid));
                gate_shop.set(None);
            }
        });
    });
    let confirm_base = base.clone();
    let confirm_toast = toast.clone();
    let confirm_modal = modal.clone();
    let on_confirmed = move |_| {
        // Payment confirmed: refresh the subscription, then enter the shop that
        // was waiting behind the gate (if any).
        let client = FetchClient {
            base_url: confirm_base.clone(),
        };
        let mut modal = confirm_modal.clone();
        let mut toast = confirm_toast.clone();
        spawn(async move {
            match client
                .subscription_current(&token().unwrap_or_default())
                .await
            {
                Ok(s) => {
                    sub.set(Some(s));
                    notify(&mut toast, SUB_CONFIRMED_MSG, false);
                    if let Some(sid) = gate_shop() {
                        gate_shop.set(None);
                        shop_id.set(Some(sid));
                    }
                    modal.close();
                }
                Err(e) => notify(&mut toast, &e, true),
            }
        });
    };
    let pay_effect_modal = modal.clone();
    let pay_effect_base = base.clone();
    let pay_effect_confirmed = on_confirmed.clone();
    use_effect(move || {
        if !pay_gate() {
            return;
        }
        pay_gate.set(false);
        let mut modal = pay_effect_modal.clone();
        modal.set_title(SUB_GATE_TITLE);
        modal.set_content(rsx! {
            SubscriptionView {
                token: token_str(),
                base: pay_effect_base.clone(),
                on_confirmed: pay_effect_confirmed.clone(),
            }
        });
        modal.open();
    });

    let open_create = {
        let create_base = base.clone();
        move |_| {
            let mut modal = modal.clone();
            new_shop.set(String::new());
            // Creation is free — the subscription gate shows after entering.
            modal.set_title("New shop");
            modal.set_content(rsx! {
                ShopForm {
                    token,
                    base: create_base.clone(),
                    shops,
                    name: new_shop,
                    on_created: move |sid: Uuid| {
                        // Active: straight in. Otherwise the gate effect opens
                        // the payment modal and keeps sid pending for entry
                        // after payment.
                        if sub_is_active() {
                            shop_id.set(Some(sid));
                        } else {
                            gate_shop.set(Some(sid));
                        }
                    },
                }
            });
            modal.open();
        }
    };

    rsx! {
        div { class: "shell",
            if shop_id().is_none() {
                div { class: "card form shop-select",
                    if shops().is_empty() {
                        div { class: "onboard",
                            div { class: "brand-mark", Icon { path: ICON_SELL, size: 34 } }
                            h2 { class: "page-title", "Create your store" }
                            p { class: "dim small", "name it — you can add goods, stock and payment right after" }
                        }
                    } else {
                        h3 { class: "doc-title", "Your stores" }
                    }
                    div { class: "shop-list",
                        for s in shops().iter().cloned() {
                            button {
                                class: "shop-chip",
                                onclick: move |_| gate_shop.set(Some(s.shop_id)),
                                "{s.name}"
                            }
                        }
                    }
                    button {
                        class: if quota_full() { "btn full" } else { "btn primary full" },
                        "aria-label": "new-shop",
                        disabled: quota_full(),
                        onclick: open_create,
                        if quota_full() {
                            "{SHOPS_QUOTA_FULL_LABEL}"
                        } else {
                            "+ New shop"
                        }
                    }
                }
            }
            if let Some(sid) = shop_id() {
                ShopPanel { token: token_str(), base: base.clone(), shop_id: sid, token_signal: token }
            }
            ModalHost {}
        }
    }
}

#[component]
fn ShopForm(
    token: Signal<Option<String>>,
    base: String,
    shops: Signal<Vec<pos_api::schema::ShopResponse>>,
    name: Signal<String>,
    on_created: EventHandler<Uuid>,
) -> Element {
    let modal = use_modal();
    let toast = use_toast();
    let busy = use_signal(|| false);
    rsx! {
        div { class: "stack",
            input {
                class: "field",
                placeholder: "new shop name",
                value: "{name}",
                oninput: move |e| name.set(e.value()),
            }
            button {
                class: "btn primary full",
                "aria-label": "create-shop",
                disabled: busy(),
                onclick: move |_| {
                    let client = FetchClient { base_url: base.clone() };
                    let token = token;
                    let mut shops = shops;
                    let mut toast = toast.clone();
                    let mut modal = modal.clone();
                    let mut busy = busy;
                    let shop_name = name.read().clone();
                    spawn(async move {
                        busy.set(true);
                        match client.create_shop(&token().unwrap_or_default(), &CreateShopRequest { name: shop_name }).await {
                            Ok(shop) => {
                                if let Ok(s) = client.list_shops(&token().unwrap_or_default()).await { shops.set(s); }
                                modal.close();
                                on_created.call(shop.shop_id);
                            }
                            Err(e) => notify(&mut toast, &e, true),
                        }
                        busy.set(false);
                    });
                },
                if busy() { "…" } else { "Create shop" }
            }
        }
    }
}

#[component]
fn ShopPanel(
    token: String,
    base: String,
    shop_id: Uuid,
    token_signal: Signal<Option<String>>,
) -> Element {
    let mut tab = use_signal(|| {
        query_param("tab")
            .and_then(|v| {
                [Tab::Sell, Tab::Goods, Tab::Reports, Tab::Settings]
                    .into_iter()
                    .find(|t| t.id() == v)
            })
            .unwrap_or(Tab::Sell)
    });
    let me = use_signal(|| Option::<pos_api::schema::MeResponse>::None);
    let shops = use_signal(Vec::<pos_api::schema::ShopResponse>::new);
    {
        let me_base = base.clone();
        let me_token = token.clone();
        let mut me = me;
        let mut shops = shops;
        use_future(move || {
            let client = FetchClient {
                base_url: me_base.clone(),
            };
            let token = me_token.clone();
            async move {
                if let Ok(m) = client.auth_me(&token).await {
                    me.set(Some(m));
                }
                if let Ok(s) = client.list_shops(&token).await {
                    shops.set(s);
                }
            }
        });
    }
    let shop_role = shops()
        .into_iter()
        .find(|s| s.shop_id == shop_id)
        .map(|s| s.your_role);
    use_effect(move || push_query_param("tab", tab().id()));
    let is_admin = match shop_role {
        Some(r) => is_admin_role(r),
        None => me().is_some_and(|m| is_admin_role(m.role)),
    };
    rsx! {
        div { class: "panel",
            Header { token_signal, base: base.clone() }
            main { class: "content",
                SubBanner { token: token.clone(), base: base.clone() }
                match tab() {
                    Tab::Sell => rsx! { SellTab { token, base, shop_id } },
                    Tab::Goods => rsx! { GoodsTab { token, base, shop_id, is_admin } },
                    Tab::Reports => rsx! { ReportsTab { token, base, shop_id } },
                    Tab::Settings => rsx! { SettingsTab { token, base, shop_id, is_admin } },
                }
            }
            nav { class: "tabbar",
                for t in [Tab::Sell, Tab::Goods, Tab::Reports, Tab::Settings] {
                    button {
                        class: if tab() == t { "tab active" } else { "tab" },
                        "aria-label": "{t.id()}",
                        onclick: move |_| tab.set(t),
                        span { class: "tab-icon", Icon { path: t.icon(), size: 22 } }
                    }
                }
            }
        }
    }
}

#[component]
fn Header(token_signal: Signal<Option<String>>, base: String) -> Element {
    let toast = use_toast();
    rsx! {
        header { class: "topbar",
            div { class: "brand-mini", Icon { path: ICON_SELL, size: 20 } }
            button {
                class: "icon-btn",
                "aria-label": "logout",
                onclick: move |_| {
                    let client = FetchClient { base_url: base.clone() };
                    let auth_token = token_signal.read().clone().unwrap_or_default();
                    let mut token = token_signal;
                    let mut toast = toast.clone();
                    ls_set(AUTH_TOKEN_KEY, "");
                    token.set(None);
                    spawn(async move {
                        if !auth_token.is_empty()
                            && let Err(e) = client.auth_logout(&auth_token).await
                        {
                            notify(&mut toast, &e, true);
                        }
                    });
                },
                Icon { path: ICON_LOGOUT, size: 20 }
            }
        }
    }
}

#[component]
fn ModalHost() -> Element {
    let modal = use_modal();
    if !modal.is_open() {
        return rsx! {};
    }
    let mut overlay_modal = modal.clone();
    let mut close_modal = modal.clone();
    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| overlay_modal.close(),
            div {
                class: "modal",
                onclick: move |e| e.stop_propagation(),
                button {
                    class: "icon-btn modal-close",
                    "aria-label": "close-modal",
                    onclick: move |_| close_modal.close(),
                    "\u{00d7}"
                }
                if !modal.get_title().is_empty() {
                    h3 { class: "doc-title", "{modal.get_title()}" }
                }
                {modal.get_content()}
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct Charge {
    billing: pos_api::schema::BillingResponse,
    expires_at: i64,
    pdf_url: String,
}

#[component]
fn SellTab(token: String, base: String, shop_id: Uuid) -> Element {
    let mut goods = use_signal(Vec::<pos_api::schema::GoodsResponse>::new);
    let mut destinations = use_signal(Vec::<pos_api::schema::DestinationResponse>::new);
    let mut selected_goods = use_signal(Vec::<CartItem>::new);
    let charge = use_signal(|| Option::<Charge>::None);
    let modal = use_modal();
    let mut now = use_signal(now_ms);
    let mut view = use_signal(|| SaleView::New);
    let mut docs = use_signal(Vec::<pos_api::schema::BillingResponse>::new);
    let mut held = use_signal(Vec::<Vec<CartItem>>::new);
    let mut scan_input = use_signal(String::new);
    let mut camera_on = use_signal(|| false);
    let mut cam_status = use_signal(|| Option::<String>::None);
    let mut shop = use_signal(|| Option::<pos_api::schema::ShopResponse>::None);
    let mut sales = use_signal(Vec::<pos_api::schema::GoodsSalesResponse>::new);
    let mut sort = use_signal(|| {
        ls_get(GOODS_SORT_KEY)
            .as_deref()
            .map(GoodsSort::parse)
            .unwrap_or(GoodsSort::Best)
    });
    let mut toast = use_toast();
    let scan_toast = toast.clone();
    let docs_toast = toast.clone();

    let base_future = base.clone();
    let token_future = token.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: base_future.clone(),
        };
        let token = token_future.clone();
        async move {
            if let Ok(g) = client.list_goods(&token, shop_id).await {
                goods.set(g);
            }
            if let Ok(d) = client.list_destinations(&token, shop_id).await {
                destinations.set(d);
            }
            if let Ok(shops) = client.list_shops(&token).await {
                shop.set(shops.into_iter().find(|s| s.shop_id == shop_id));
            }
            if let Ok(s) = client.list_goods_sales(&token, shop_id).await {
                sales.set(s);
            }
        }
    });

    use_future(move || async move {
        loop {
            now.set(now_ms());
            TimeoutFuture::new(TICK_MS as u32).await;
        }
    });

    let line_total = |id: Uuid| -> rust_decimal::Decimal {
        let qty = selected_goods()
            .iter()
            .find(|c| c.goods_id == id)
            .map(|c| c.qty)
            .unwrap_or(CART_QTY_STEP);
        goods()
            .iter()
            .find(|g| g.goods_id == id)
            .map(|g| g.price * rust_decimal::Decimal::from(qty))
            .unwrap_or_default()
    };
    let subtotal: rust_decimal::Decimal = selected_goods()
        .iter()
        .map(|c| line_total(c.goods_id))
        .sum();
    let shop_cfg = shop();
    let (vat_mode, vat_rate, fx, shop_currency) = shop_cfg
        .map(|s| (s.vat_mode, s.vat_rate, s.fx_thb_per_usd, s.currency))
        .unwrap_or((
            pos_api::schema::VatMode::None,
            rust_decimal::Decimal::ZERO,
            rust_decimal::Decimal::ZERO,
            pos_api::schema::Currency::Usdt,
        ));
    let sym = if shop_currency == pos_api::schema::Currency::Usdt {
        "$"
    } else {
        "\u{0e3f}"
    };
    let hundred = rust_decimal::Decimal::new(100, 0);
    let vat_amount = match vat_mode {
        pos_api::schema::VatMode::None => rust_decimal::Decimal::ZERO,
        pos_api::schema::VatMode::Exclusive => (subtotal * vat_rate / hundred).round_dp(2),
        pos_api::schema::VatMode::Inclusive => {
            (subtotal * vat_rate / (hundred + vat_rate)).round_dp(2)
        }
    };
    let total = match vat_mode {
        pos_api::schema::VatMode::Exclusive => subtotal + vat_amount,
        _ => subtotal,
    };
    let held_labels: Vec<String> = held()
        .iter()
        .enumerate()
        .map(|(i, items)| {
            format!(
                "{} {} \u{00b7} {} {}",
                HELD_LABEL_PREFIX,
                i + 1,
                items.len(),
                HELD_ITEMS_SUFFIX
            )
        })
        .collect();
    let mut picked_destination = use_signal(|| Option::<Uuid>::None);
    let destinations_now = destinations();
    let cart_count = selected_goods().len();
    let destination_id = picked_destination()
        .filter(|id| destinations_now.iter().any(|d| &d.destination_id == id))
        .or_else(|| destinations_now.first().map(|d| d.destination_id));

    rsx! {
        div { class: "stack sell-stack",
            div { class: "settings-tabs",
                    button {
                        class: if view() == SaleView::New { "settings-tab active" } else { "settings-tab" },
                        onclick: move |_| view.set(SaleView::New),
                        "{SaleView::NEW_LABEL}"
                    }
                    button {
                        class: if view() == SaleView::Docs { "settings-tab active" } else { "settings-tab" },
                        onclick: move |_| {
                            view.set(SaleView::Docs);
                            let client = FetchClient { base_url: base.clone() };
                            let token = token.clone();
                            let mut docs_toast = docs_toast.clone();
                            spawn(async move {
                                match client.list_billings(&token, shop_id).await {
                                    Ok(d) => docs.set(d),
                                    Err(e) => notify(&mut docs_toast, &e, true),
                                }
                            });
                        },
                        "{SaleView::DOCS_LABEL}"
                    }
                }
                match view() {
                    SaleView::New => rsx! {
                h2 { class: "page-title", "New sale" }

                if !held().is_empty() {
                    div { class: "dest-strip",
                        span { class: "dim small", "{HELD_STRIP_LABEL}" }
                        for (i, label) in held_labels.iter().enumerate() {
                            button {
                                class: "dest-chip held-chip",
                                "aria-label": "held-bill-{i}",
                                onclick: move |_| {
                                    let mut cur = held().clone();
                                    let bill = cur.remove(i);
                                    held.set(cur);
                                    selected_goods.set(bill);
                                },
                                "{label}"
                            }
                            button {
                                class: "icon-btn",
                                "aria-label": "discard-held-{i}",
                                onclick: move |_| {
                                    let mut cur = held().clone();
                                    cur.remove(i);
                                    held.set(cur);
                                },
                                "\u{00d7}"
                            }
                        }
                    }
                }
                div { class: "stack",
                    input {
                        class: "field",
                        placeholder: SCAN_PLACEHOLDER,
                        value: "{scan_input}",
                        oninput: move |e| scan_input.set(e.value()),
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key() != Key::Enter {
                                return;
                            }
                            let code = pos_barcode::format::normalize(&scan_input());
                            scan_input.set(String::new());
                            if code.is_empty() {
                                return;
                            }
                            let mut cur = selected_goods().clone();
                            if !add_by_barcode(&goods(), &code, &mut cur) {
                                match name_matches(&goods(), &code).as_slice() {
                                    [id] => {
                                        match cur.iter_mut().find(|c| c.goods_id == *id) {
                                            Some(item) => item.bump(CART_QTY_STEP),
                                            None => cur.push(CartItem::new(*id)),
                                        }
                                    }
                                    [] => notify(
                                        &mut toast,
                                        &format!("{SEARCH_NO_MATCH_MSG}{code}"),
                                        true,
                                    ),
                                    _ => notify(
                                        &mut toast,
                                        &format!("{SEARCH_MULTI_MATCH_MSG}{code}"),
                                        true,
                                    ),
                                }
                            }
                            selected_goods.set(cur);
                        },
                    }
                    div { class: "scan-row",
                        button {
                            class: if camera_on() { "scan-toggle on" } else { "scan-toggle" },
                            onclick: move |_| {
                                if camera_on() {
                                    camera_on.set(false);
                                    return;
                                }
                                if !camera_supported() {
                                    cam_status.set(Some(String::from(CAMERA_UNSUPPORTED_MSG)));
                                    return;
                                }
                                cam_status.set(None);
                                camera_on.set(true);
                                let t = scan_toast.clone();
                                spawn(run_camera_scanner(
                                    camera_on,
                                    goods,
                                    selected_goods,
                                    t,
                                ));
                            },
                            if camera_on() { "{CAMERA_STOP_LABEL}" } else { "{CAMERA_START_LABEL}" }
                        }
                        if let Some(s) = cam_status() {
                            span { class: "dim small", "{s}" }
                        }
                    }
                    if camera_on() {
                        div { class: "cam-box",
                            video {
                                id: CAMERA_VIDEO_ID,
                                class: "cam-video",
                                autoplay: "true",
                                playsinline: "true",
                                muted: "true",
                            }
                        }
                    }
                    p { class: "dim small", "type or scan a barcode to add — or tap the tiles" }
                }

                div { class: "stack",
                        div { class: "row split",
                            select {
                                class: "field sort-select",
                                "aria-label": "goods-sort",
                                value: "{sort().key()}",
                                onchange: move |e| {
                                    let s = GoodsSort::parse(&e.value());
                                    sort.set(s);
                                    ls_set(GOODS_SORT_KEY, s.key());
                                },
                                for s in GoodsSort::ALL {
                                    option { value: "{s.key()}", "{s.label()}" }
                                }
                            }
                        }
                        div { class: "goods-grid",
                            for g in sorted_sell_goods(&search_filtered(&goods(), &scan_input()), &sales(), sort()) {
                                button {
                                    class: if selected_goods().iter().any(|c| c.goods_id == g.goods_id) { "goods-tile picked" } else { "goods-tile" },
                                    onclick: move |_| {
                                        let mut cur = selected_goods().clone();
                                        if let Some(pos) = cur.iter().position(|c| c.goods_id == g.goods_id) {
                                            cur.remove(pos);
                                        } else {
                                            cur.push(CartItem::new(g.goods_id));
                                        }
                                        selected_goods.set(cur);
                                    },
                                    {goods_icon_of(&g)}
                                    span { class: "goods-name", "{g.name}" }
                                    if let Some(b) = &g.barcode {
                                        span { class: "dim small", "{b}" }
                                    }
                                    span { class: "goods-meta",
                                        span { class: "goods-price", "{sym} {g.price}" }
                                        if g.units > 0 {
                                            span { class: if g.units <= LOW_STOCK { "unit-chip warn" } else { "unit-chip" }, "x{g.units}" }
                                        }
                                    }
                                }
                            }
                            if goods().is_empty() {
                                p { class: "dim", "no goods — add them in the Goods tab" }
                            }
                        }
                }
                button {
                    class: "fab cart",
                    "aria-label": "open-cart",
                    disabled: "{selected_goods().is_empty()}",
                    onclick: {
                        let cart_modal = modal.clone();
                        move |_| {
                            let mut modal = cart_modal.clone();
                            modal.set_title(CART_TITLE);
                            modal.set_content(rsx! {
                                CartModal { selected_goods, goods, sym }
                            });
                            modal.open();
                        }
                    },
                    Icon { path: ICON_CART, size: 24 }
                    if cart_count > 0 {
                        span { class: "fab-badge", "{cart_count}" }
                    }
                }
                button {
                    class: "fab hold",
                    "aria-label": "hold-bill",
                    disabled: "{selected_goods().is_empty() || held().len() >= HELD_BILL_MAX}",
                    onclick: move |_| {
                        let mut cur = held().clone();
                        cur.push(selected_goods().clone());
                        held.set(cur);
                        selected_goods.set(Vec::new());
                    },
                    Icon { path: ICON_HOLD, size: 24 }
                }
                if total > rust_decimal::Decimal::ZERO {
                    div { class: "total-bar",
                        if vat_mode != pos_api::schema::VatMode::None {
                            span { class: "dim small",
                                "Subtotal {sym}{subtotal.normalize()} \u{00b7} VAT {vat_rate.normalize()}% {sym}{vat_amount.normalize()}"
                            }
                        }
                        if destinations().len() > 1 {
                            select {
                                class: "field",
                                "aria-label": "receive-account",
                                value: "{destination_id.map(|d| d.to_string()).unwrap_or_default()}",
                                onchange: move |e| {
                                    picked_destination.set(e.value().parse::<Uuid>().ok());
                                },
                                for d in destinations().iter().cloned() {
                                    option { value: "{d.destination_id}", "{dest_label(&d)}" }
                                }
                            }
                        }
                        span { "Total" }
                        span { class: "pay-total small-total", "{sym} {total.normalize()}" }
                    }
                }
                button {
                    class: "fab",
                    "aria-label": "new-doc",
                    disabled: "{selected_goods().is_empty() || destinations().is_empty()}",
                    onclick: {
                        let modal = modal.clone();
                        let base = base.clone();
                        let token = token.clone();
                        move |_| {
                            let cart_items = selected_goods()
                                .iter()
                                .map(|c| pos_api::schema::CheckoutItem {
                                    goods_id: c.goods_id,
                                    qty: c.qty,
                                })
                                .collect();
                            let mut modal = modal.clone();
                            modal.set_title(PAY_METHOD_TITLE);
                            modal.set_content(rsx! {
                                MethodSelect {
                                    shop_id,
                                    base: base.clone(),
                                    token: token.clone(),
                                    items: cart_items,
                                    destinations: destinations.read().clone(),
                                    total,
                                    currency: shop_currency,
                                    fx,
                                    selected_goods,
                                    goods,
                                    charge,
                                    now,
                                    picked_destination,
                                }
                            });
                            modal.open();
                        }
                    },
                    Icon { path: ICON_ADD, size: 28 }
                }
                },
                    SaleView::Docs => rsx! {
                        DocumentsList { docs, base: base.clone(), sym, charge, now }
                    },
                }
        }
    }
}

#[component]
fn CartModal(
    selected_goods: Signal<Vec<CartItem>>,
    goods: Signal<Vec<pos_api::schema::GoodsResponse>>,
    sym: &'static str,
) -> Element {
    let mut modal = use_modal();
    let count = selected_goods().len();
    rsx! {
        div { class: "stack",
            div { class: "cart-head",
                span { class: "dim small", "{count} {CART_ITEMS_WORD}" }
                span { class: "spacer" }
                button {
                    class: "cart-clear",
                    "aria-label": "clear-cart",
                    disabled: "{selected_goods().is_empty()}",
                    onclick: move |_| selected_goods.set(Vec::new()),
                    "{CART_CLEAR_LABEL}"
                }
            }
            if selected_goods().is_empty() {
                p { class: "dim pad", "{CART_EMPTY_MSG}" }
            }
            div { class: "stack cart-list",
                for item in selected_goods().iter().cloned() {
                    CartRow { item, selected_goods, goods, sym }
                }
            }
            button {
                class: "btn ghost full",
                "aria-label": "close-cart",
                onclick: move |_| modal.close(),
                "close"
            }
        }
    }
}

#[component]
fn CartRow(
    item: CartItem,
    selected_goods: Signal<Vec<CartItem>>,
    goods: Signal<Vec<pos_api::schema::GoodsResponse>>,
    sym: &'static str,
) -> Element {
    let modal = use_modal();
    let goods_item = goods()
        .iter()
        .find(|g| g.goods_id == item.goods_id)
        .cloned();
    let name = goods_item
        .as_ref()
        .map(|g| g.name.clone())
        .unwrap_or_default();
    let price = goods_item.map(|g| g.price).unwrap_or_default();
    let line = price * rust_decimal::Decimal::from(item.qty);
    let row_modal = modal.clone();
    let row_name = name.clone();
    rsx! {
        div { class: "row split cart-row",
            span { class: "goods-name", "{name} "
                span { class: "dim small", "x{item.qty}" }
            }
            span { class: "row cart-qty",
                span { class: "goods-price", "{sym} {line.normalize()}" }
                button {
                    class: "btn-circle",
                    "aria-label": "edit-qty",
                    onclick: move |_| {
                        let mut modal = row_modal.clone();
                        modal.set_title(row_name.as_str());
                        modal.set_content(rsx! {
                            QtyEdit { goods_id: item.goods_id, selected_goods }
                        });
                        modal.open();
                    },
                    Icon { path: ICON_QTY, size: 18 }
                }
            }
        }
    }
}

#[component]
fn QtyEdit(goods_id: Uuid, selected_goods: Signal<Vec<CartItem>>) -> Element {
    let mut modal = use_modal();
    let mut qty = use_signal(|| {
        selected_goods()
            .iter()
            .find(|c| c.goods_id == goods_id)
            .map(|c| c.qty)
            .unwrap_or(CART_QTY_STEP)
    });
    let save = move |_| {
        let mut cur = selected_goods().clone();
        if let Some(item) = cur.iter_mut().find(|c| c.goods_id == goods_id) {
            item.qty = qty();
        }
        selected_goods.set(cur);
        modal.close();
    };
    rsx! {
        div { class: "stack",
            div { class: "qty-edit",
                button {
                    class: "qty-btn qty-btn-lg",
                    "aria-label": "qty-dec",
                    disabled: "{qty() <= CART_QTY_STEP}",
                    onclick: move |_| qty.set((qty()).saturating_sub(CART_QTY_STEP)),
                    "−"
                }
                span { class: "qty-num qty-num-lg", "x{qty}" }
                button {
                    class: "qty-btn qty-btn-lg",
                    "aria-label": "qty-inc",
                    onclick: move |_| qty.set(qty() + CART_QTY_STEP),
                    "+"
                }
            }
            button {
                class: "btn primary full",
                "aria-label": "qty-done",
                onclick: save,
                "{QTY_DONE_LABEL}"
            }
        }
    }
}

#[component]
fn DocumentsList(
    docs: Signal<Vec<pos_api::schema::BillingResponse>>,
    base: String,
    sym: &'static str,
    charge: Signal<Option<Charge>>,
    now: Signal<i64>,
) -> Element {
    rsx! {
        div { class: "stack",
            h2 { class: "page-title", "{SaleView::DOCS_LABEL}" }
            div { class: "list",
                for inv in docs().iter().cloned() {
                    DocumentRow { inv, base: base.clone(), sym, charge, now }
                }
                if docs().is_empty() {
                    p { class: "dim pad", "no documents yet" }
                }
            }
        }
    }
}

#[component]
fn DocumentRow(
    inv: pos_api::schema::BillingResponse,
    base: String,
    sym: &'static str,
    charge: Signal<Option<Charge>>,
    now: Signal<i64>,
) -> Element {
    let modal = use_modal();
    let show = move |v: rust_decimal::Decimal| format!("{sym}{}", v.round_dp(2).normalize());
    rsx! {
        div {
            class: "list-row clickable",
            "aria-label": "document-row",
            onclick: {
                let billing = inv.clone();
                let detail_base = base.clone();
                let mut row_modal = modal.clone();
                move |_| {
                    row_modal.set_title(DETAIL_TITLE);
                    row_modal.set_content(rsx! { BillingDetail { billing: billing.clone(), base: detail_base.clone(), charge, now } });
                    row_modal.open();
                }
            },
            div { class: "goods-col",
                span { class: "goods-name", {inv.lines.first().map(|l| l.name.clone()).unwrap_or_default()} }
                span { class: "dim small", "{chain_label(inv.destination.chain)} · margin {show(inv.total - inv.cost)}" }
            }
            div { class: "goods-col right",
                span { class: "goods-price", "{show(inv.total)}" }
                button {
                    class: "dim small pdf-link",
                    "aria-label": "show-qr",
                    onclick: {
                        let billing = inv.clone();
                        let base = base.clone();
                        let mut modal = modal.clone();
                        move |e: dioxus::prelude::MouseEvent| {
                            e.stop_propagation();
                            let pdf_url = format!("{}{}", base.trim_end_matches('/'), billing.pdf_url);
                            charge.set(Some(Charge {
                                billing: billing.clone(),
                                expires_at: now_ms() + QR_TTL_MS,
                                pdf_url,
                            }));
                            modal.set_title(BILLING_TITLE);
                            modal.set_content(rsx! { QrModalContent { charge, now } });
                            modal.open();
                        }
                    },
                    "QR"
                }
                a { class: "dim small pdf-link", href: "{base}{inv.pdf_url}", target: "_blank", onclick: move |e: dioxus::prelude::MouseEvent| e.stop_propagation(), "PDF" }
            }
        }
    }
}

#[component]
fn BillingDetail(
    billing: pos_api::schema::BillingResponse,
    base: String,
    charge: Signal<Option<Charge>>,
    now: Signal<i64>,
) -> Element {
    let mut modal = use_modal();
    let dest = billing.destination.clone();
    let pdf_url = format!("{}{}", base.trim_end_matches('/'), billing.pdf_url);
    let qr_pdf_url = pdf_url.clone();
    let show = |v: rust_decimal::Decimal| format!("${}", v.round_dp(2).normalize());
    rsx! {
        div { class: "stack",
            for line in billing.lines.iter() {
                div { class: "list-row",
                    div { class: "goods-col",
                        span { class: "goods-name", "{line.name}" }
                        span { class: "dim small", "{line.qty} x {show(line.price)}" }
                    }
                    div { class: "goods-col right",
                        span { class: "goods-price", "{show(line.price * rust_decimal::Decimal::from(line.qty))}" }
                    }
                }
            }
            div { class: "stack pad",
                p { class: "dim small", "Subtotal {show(billing.subtotal)} · VAT {billing.vat_rate.normalize()}% {show(billing.vat_amount)}" }
                p { class: "dim small", "Pay with {chain_label(dest.chain)} · {truncate(&dest.address, TRUNCATE_ADDR)}" }
                p { class: "dim small", "Margin {show(billing.total - billing.cost)}" }
            }
            button {
                class: "btn ghost full",
                onclick: move |_| {
                    charge.set(Some(Charge {
                        billing: billing.clone(),
                        expires_at: now_ms() + QR_TTL_MS,
                        pdf_url: qr_pdf_url.clone(),
                    }));
                    modal.set_title(BILLING_TITLE);
                    modal.set_content(rsx! { QrModalContent { charge, now } });
                    modal.open();
                },
                "Show QR"
            }
            a { class: "btn ghost full pdf-link", href: "{pdf_url}", target: "_blank", "Download PDF" }
        }
    }
}

#[component]
fn MethodSelect(
    shop_id: Uuid,
    base: String,
    token: String,
    items: Vec<pos_api::schema::CheckoutItem>,
    destinations: Vec<pos_api::schema::DestinationResponse>,
    total: rust_decimal::Decimal,
    currency: pos_api::schema::Currency,
    fx: rust_decimal::Decimal,
    selected_goods: Signal<Vec<CartItem>>,
    goods: Signal<Vec<pos_api::schema::GoodsResponse>>,
    charge: Signal<Option<Charge>>,
    now: Signal<i64>,
    picked_destination: Signal<Option<Uuid>>,
) -> Element {
    let modal = use_modal();
    let sym = if currency == pos_api::schema::Currency::Usdt {
        "$"
    } else {
        "\u{0e3f}"
    };
    let bank_amount =
        if currency == pos_api::schema::Currency::Usdt && fx > rust_decimal::Decimal::ZERO {
            (total * fx).round_dp(2).normalize()
        } else {
            total.normalize()
        };
    let token_amount =
        if currency == pos_api::schema::Currency::Thb && fx > rust_decimal::Decimal::ZERO {
            (total / fx).round_dp(TOKEN_AMOUNT_DP).normalize()
        } else {
            total.normalize()
        };
    rsx! {
        div { class: "stack",
            div { class: "pay-total", "{sym} {total.normalize()}" }
            div { class: "method-sheet",
                for d in destinations.iter().cloned() {
                    {
                        let dest_id = d.destination_id;
                        let chain = d.chain;
                        let addr = d.address.clone();
                        let tile_base = base.clone();
                        let tile_token = token.clone();
                        let tile_items = items.clone();
                        let is_bank = chain == pos_api::schema::Chain::Bank;
                        let amount = if is_bank {
                            format!("\u{0e3f}{bank_amount}")
                        } else {
                            format!("{token_amount} {}", token_label(d.token))
                        };
                        rsx! {
                            button {
                                class: "method-tile",
                                "aria-label": "pay-method",
                                onclick: {
                                    let mut modal = modal.clone();
                                    move |_| {
                                    picked_destination.set(Some(dest_id));
                                    modal.set_content(rsx! {
                                        CheckoutContent {
                                            shop_id,
                                            base: tile_base.clone(),
                                            token: tile_token.clone(),
                                            goods_items: tile_items.clone(),
                                            destination_id: dest_id,
                                            selected_goods,
                                            goods,
                                            charge,
                                            now,
                                        }
                                    });
                                    }
                                },
                                span { class: "chain", "{chain_label(chain)}" }
                                span { class: "dim small", "{truncate(&addr, TRUNCATE_ADDR)}" }
                                span { class: "goods-price", "{amount}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CheckoutContent(
    shop_id: Uuid,
    base: String,
    token: String,
    goods_items: Vec<pos_api::schema::CheckoutItem>,
    destination_id: Uuid,
    selected_goods: Signal<Vec<CartItem>>,
    goods: Signal<Vec<pos_api::schema::GoodsResponse>>,
    charge: Signal<Option<Charge>>,
    now: Signal<i64>,
) -> Element {
    let modal = use_modal();
    let toast = use_toast();
    use_future(move || {
        let mut modal = modal.clone();
        let mut toast = toast.clone();
        let base = base.clone();
        let token = token.clone();
        let goods_items = goods_items.clone();
        async move {
            let client = FetchClient {
                base_url: base.clone(),
            };
            match client
                .checkout(
                    &token,
                    &CheckoutRequest {
                        shop_id,
                        goods_ids: Vec::new(),
                        items: goods_items.clone(),
                        destination_id,
                    },
                )
                .await
            {
                Ok(inv) => {
                    selected_goods.set(Vec::new());
                    if let Ok(g) = client.list_goods(&token, shop_id).await {
                        goods.set(g);
                    }
                    let pdf_url = format!("{}{}", base.trim_end_matches('/'), inv.pdf_url);
                    charge.set(Some(Charge {
                        billing: inv,
                        expires_at: now_ms() + QR_TTL_MS,
                        pdf_url,
                    }));
                    modal.set_title(BILLING_TITLE);
                    modal.set_content(rsx! { QrModalContent { charge, now } });
                    modal.open();
                }
                Err(e) => {
                    notify(&mut toast, &e, true);
                    modal.close();
                }
            }
        }
    });
    rsx! {
        div { class: "stack center",
            p { class: "dim", "creating document…" }
        }
    }
}

#[component]
fn QrModalContent(charge: Signal<Option<Charge>>, now: Signal<i64>) -> Element {
    let mut modal = use_modal();
    let Some(c) = charge().filter(|c| now() < c.expires_at) else {
        return rsx! {
            div { class: "stack center",
                if charge().is_some() {
                    div { class: "card expired", p { "QR expired" } }
                }
                button {
                    class: "btn ghost full",
                    onclick: move |_| {
                        charge.set(None);
                        modal.close();
                    },
                    "New sale"
                }
            }
        };
    };
    rsx! {
        QrPay { charge: c, now_ms: now }
    }
}

#[component]
fn QrPay(charge: Charge, now_ms: Signal<i64>) -> Element {
    let inv = &charge.billing;
    let left = charge.expires_at - now_ms();
    let svg = qr_svg(&inv.qr_payload);
    let qr_el = match svg {
        Some(svg) => rsx! { div { class: "qr", dangerous_inner_html: "{svg}" } },
        None => rsx! { p { class: "error", "QR render failed" } },
    };
    let pdf_url = charge.pdf_url.clone();
    let token = token_label(inv.destination.token);
    rsx! {
        div { class: "card pay",
            if inv.destination.chain == pos_api::schema::Chain::Bank {
                p { class: "pay-total", "\u{0e3f} {inv.qr_amount.normalize()}" }
            } else {
                p { class: "pay-total", "{inv.qr_amount.normalize()} {token}" }
            }
            if inv.vat_mode != pos_api::schema::VatMode::None {
                p { class: "dim small", "Subtotal {inv.subtotal.normalize()} \u{00b7} VAT {inv.vat_rate.normalize()}% {inv.vat_amount.normalize()}" }
            }
            {qr_el}
            p { class: "pay-status",
                span { class: "pay-dot" }
                "waiting for payment"
            }
            p { class: if left < URGENCY_MS { "countdown urgent" } else { "countdown" }, "{fmt_remaining(left)}" }
            p { class: "dim small", "scan to pay · {chain_label(inv.destination.chain)}" }
            a { class: "btn ghost pdf-link", href: "{pdf_url}", target: "_blank", "Download PDF" }
        }
    }
}

fn sub_banner_text(sub: &SubscriptionResponse) -> Option<(&'static str, String)> {
    match sub.status {
        SubscriptionStatusApi::Active => {
            let expires = sub.expires_at?;
            let days =
                (expires.timestamp() - js_sys::Date::now() as i64 / 1000).div_euclid(86_400) + 1;
            (days <= SUB_NEAR_EXPIRY_DAYS).then(|| {
                (
                    BANNER_EXPIRING,
                    format!("{SUB_EXPIRING_PREFIX}{days}{SUB_EXPIRING_SUFFIX}"),
                )
            })
        }
        SubscriptionStatusApi::Grace => Some(("banner warn", SUB_GRACE_MSG.to_string())),
        SubscriptionStatusApi::Lapsed | SubscriptionStatusApi::Deleted => {
            Some(("banner err", SUB_LAPSED_MSG.to_string()))
        }
    }
}

fn fmt_local(t: chrono::DateTime<chrono::Utc>) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(t.timestamp_millis() as f64));
    d.to_string().as_string().unwrap_or_default()
}

fn wallet_label(busy: Signal<bool>, name: &str) -> String {
    if *busy.read() {
        String::from("…")
    } else {
        format!("Connect {name}")
    }
}

fn copy_text(s: &str) {
    if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
        let _ = nav.clipboard().write_text(s);
    }
}

fn query_param(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let query = search.strip_prefix('?').unwrap_or(&search);
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

fn push_query_param(key: &str, value: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let search = window.location().search().unwrap_or_default();
    let query = search.strip_prefix('?').unwrap_or(&search);
    let mut pairs: Vec<String> = query
        .split('&')
        .filter(|p| !p.is_empty() && p.split_once('=').map(|(k, _)| k != key).unwrap_or(true))
        .map(str::to_string)
        .collect();
    pairs.push(format!("{key}={value}"));
    let url = format!("?{}", pairs.join("&"));
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
    }
}

#[component]
fn QuotaBar(label: &'static str, used: i64, quota: i64) -> Element {
    let pct = if quota <= 0 {
        0
    } else {
        (used * 100 / quota).clamp(0, 100)
    };
    rsx! {
        div { class: "quota",
            div { class: "row split small dim",
                span { "{label}" }
                span { "{used}/{quota}" }
            }
            div { class: "bar",
                div { class: "bar-fill", width: "{pct}%" } }
        }
    }
}

#[component]
fn SubBanner(token: String, base: String) -> Element {
    let sub = use_signal(|| Option::<SubscriptionResponse>::None);
    let toast = use_toast();
    let banner_base = base.clone();
    let banner_token = token.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: banner_base.clone(),
        };
        let token = banner_token.clone();
        let mut sub = sub;
        let mut toast = toast.clone();
        async move {
            match client.subscription_current(&token).await {
                Ok(s) => sub.set(Some(s)),
                Err(e) if !e.contains(HTTP_FORBIDDEN) => notify(&mut toast, &e, true),
                Err(_) => {}
            }
        }
    });
    let Some(s) = sub() else {
        return rsx! {};
    };
    rsx! {
        div {
            if let Some((class, msg)) = sub_banner_text(&s) {
                div { class: "{class}", "{msg}" }
            }
        }
    }
}

#[component]
fn QuotaUsage(token: String, base: String) -> Element {
    let sub = use_signal(|| Option::<SubscriptionResponse>::None);
    let toast = use_toast();
    let usage_base = base.clone();
    let usage_token = token.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: usage_base.clone(),
        };
        let token = usage_token.clone();
        let mut sub = sub;
        let mut toast = toast.clone();
        async move {
            match client.subscription_current(&token).await {
                Ok(s) => sub.set(Some(s)),
                Err(e) if !e.contains(HTTP_FORBIDDEN) => notify(&mut toast, &e, true),
                Err(_) => {}
            }
        }
    });
    let Some(s) = sub() else {
        return rsx! {};
    };
    rsx! {
        div { class: "quota-row",
            QuotaBar { label: "shops", used: s.shops_used as i64, quota: s.shops_quota as i64 }
            QuotaBar { label: "goods", used: s.goods_used as i64, quota: s.goods_quota as i64 }
            QuotaBar { label: "storage (MB)", used: s.storage_bytes / BYTES_PER_MB, quota: s.storage_quota_bytes / BYTES_PER_MB }
        }
    }
}

fn is_admin_role(role: pos_api::schema::Role) -> bool {
    matches!(
        role,
        pos_api::schema::Role::SuperAdmin
            | pos_api::schema::Role::Owner
            | pos_api::schema::Role::Admin
    )
}

fn fmt_bytes(bytes: i64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= BYTES_PER_KB && unit < BYTE_UNITS.len() - 1 {
        value /= BYTES_PER_KB;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", BYTE_UNITS[0])
    } else {
        format!("{value:.1} {}", BYTE_UNITS[unit])
    }
}

fn short_key(key: Uuid) -> String {
    let s = key.to_string();
    String::from(&s[..KEY_SHORT_LEN.min(s.len())])
}

#[component]
fn StorageView(token: String, base: String, shop_id: Uuid, is_admin: bool) -> Element {
    let mut objects = use_signal(Vec::<StorageObjectResponse>::new);
    let mut version = use_signal(|| 0u32);
    let toast = use_toast();
    let ver = version.read().to_owned();
    let load_base = base.clone();
    let load_token = token.clone();
    let load_toast = toast.clone();
    use_resource(use_reactive!(|(ver,)| {
        let client = FetchClient {
            base_url: load_base.clone(),
        };
        let token = load_token.clone();
        let mut toast = load_toast.clone();
        async move {
            let _ = ver;
            match client.list_storage_objects(&token, shop_id).await {
                Ok(o) => objects.set(o),
                Err(e) => notify(&mut toast, &e, true),
            }
        }
    }));
    let count = objects().len();
    let total: i64 = objects().iter().map(|o| o.bytes).sum();
    rsx! {
        div { class: "stack",
            div { class: "row split",
                span { class: "dim small", "{count} objects · {fmt_bytes(total)}" }
                button {
                    class: "btn ghost",
                    "aria-label": "refresh-storage",
                    onclick: move |_| version += 1,
                    "{STORAGE_REFRESH_LABEL}"
                }
            }
            if count == 0 {
                p { class: "dim pad", "{STORAGE_EMPTY_MSG}" }
            }
            for o in objects().iter().cloned() {
                div { class: "row split",
                    span { class: "mono small", "{short_key(o.key)}" }
                    div { class: "row",
                        span { class: "dim small", "{fmt_bytes(o.bytes)}" }
                        if is_admin {
                            button {
                                class: "btn ghost",
                                "aria-label": "delete-storage-{o.key}",
                                onclick: {
                                    let base = base.clone();
                                    let token = token.clone();
                                    let toast = toast.clone();
                                    move |_| {
                                        let client = FetchClient { base_url: base.clone() };
                                        let token = token.clone();
                                        let mut toast = toast.clone();
                                        let mut version = version;
                                        let key = o.key;
                                        spawn(async move {
                                            match client.delete_storage_object(&token, shop_id, key).await {
                                                Ok(()) => version += 1,
                                                Err(e) => notify(&mut toast, &e, true),
                                            }
                                        });
                                    }
                                },
                                "{GOODS_DELETE_LABEL}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PayPending(
    recipient: String,
    amount: String,
    mint: String,
    sig_input: Signal<String>,
    on_verify: EventHandler,
) -> Element {
    let pay_url = format!("solana:{recipient}?amount={amount}&spl-token={mint}");
    let svg = qr_svg(&pay_url);
    rsx! {
        div { class: "card stack",
            div { class: "pay-status", span { class: "pay-dot" } span { {SUB_PENDING_MSG} } }
            match svg {
                Some(svg) => rsx! { div { class: "qr", dangerous_inner_html: "{svg}" } },
                None => rsx! { p { class: "error", "QR render failed" } },
            }
            p { class: "dim small", {SUB_MERCHANT_LABEL} }
            div { class: "row split",
                span { class: "mono small", "{truncate(&recipient, TRUNCATE_ADDR)}" }
                button {
                    class: "btn ghost",
                    onclick: move |_| copy_text(&recipient),
                    {SUB_COPY_LABEL}
                }
            }
            p { class: "small", "amount: {amount} USDC" }
            a { class: "pdf-link mono small", href: "{pay_url}", "open in wallet" }
            input {
                class: "field mono",
                placeholder: SUB_SIG_PLACEHOLDER,
                value: "{sig_input}",
                oninput: move |e| sig_input.set(e.value()),
            }
            button {
                class: "btn primary full",
                disabled: "{sig_input().is_empty()}",
                onclick: move |_| on_verify.call(()),
                {SUB_VERIFY_LABEL}
            }
        }
    }
}

#[component]
fn SubscriptionView(token: String, base: String, on_confirmed: EventHandler) -> Element {
    let sub = use_signal(|| Option::<SubscriptionResponse>::None);
    let order = use_signal(|| Option::<PaymentOrderResponse>::None);
    let mut pay = use_signal(|| PayState::Idle);
    let mut period = use_signal(|| BillingPeriodApi::Monthly);
    let pay_err = use_signal(String::new);
    let sig_input = use_signal(String::new);
    let busy = use_signal(|| false);
    let toast = use_toast();
    let fut_base = base.clone();
    let fut_token = token.clone();

    use_future(move || {
        let client = FetchClient {
            base_url: fut_base.clone(),
        };
        let token = fut_token.clone();
        let mut sub = sub;
        async move {
            if let Ok(s) = client.subscription_current(&token).await {
                sub.set(Some(s));
            }
        }
    });

    let order_base = base.clone();
    let order_token = token.clone();
    let order_toast = toast.clone();
    let start_order = move |renew: bool| {
        let client = FetchClient {
            base_url: order_base.clone(),
        };
        let token = order_token.clone();
        let mut busy = busy;
        let mut order = order;
        let mut pay = pay;
        let mut pay_err = pay_err;
        let mut toast = order_toast.clone();
        async move {
            busy.set(true);
            match client
                .create_payment_order(
                    &token,
                    &CreatePaymentOrderRequest {
                        renew,
                        period: Some(period()),
                    },
                )
                .await
            {
                Ok(o) => {
                    order.set(Some(o));
                    pay.set(PayState::Pending);
                    pay_err.set(String::new());
                }
                Err(e) => notify(&mut toast, &e, true),
            }
            busy.set(false);
        }
    };

    let verify_base = base.clone();
    let verify_token = token.clone();
    let do_verify = move |_| {
        let client = FetchClient {
            base_url: verify_base.clone(),
        };
        let mut sub = sub;
        let mut pay = pay;
        let mut pay_err = pay_err;
        let mut sig_input = sig_input;
        let mut toast = toast.clone();
        let token = verify_token.clone();
        let order = order.read().clone();
        let signature = sig_input.read().clone();
        spawn(async move {
            let Some(o) = order else { return };
            if signature.is_empty() {
                return;
            }
            pay.set(PayState::Confirming);
            match client
                .verify_payment(
                    &token,
                    &VerifyPaymentRequest {
                        order_id: o.order_id,
                        signature,
                    },
                )
                .await
            {
                Ok(s) => {
                    sub.set(Some(s));
                    pay.set(PayState::Confirmed);
                    sig_input.set(String::new());
                    notify(&mut toast, SUB_CONFIRMED_MSG, false);
                    on_confirmed.call(());
                }
                Err(e) => {
                    pay_err.set(e);
                    pay.set(PayState::Failed);
                }
            }
        });
    };

    let renewing = matches!(
        sub().map(|s| s.status),
        Some(SubscriptionStatusApi::Active) | Some(SubscriptionStatusApi::Grace)
    );
    let price_label = match period() {
        BillingPeriodApi::Monthly => SUB_PRICE_MONTHLY_LABEL,
        BillingPeriodApi::Yearly => SUB_PRICE_YEARLY_LABEL,
    };

    rsx! {
        div { class: "stack",
            h2 { class: "page-title", "Subscription" }
            div { class: "card stack",
                if let Some(s) = sub() {
                    div { class: "row split small dim",
                        span { "{s.status:?}" }
                        if let Some(exp) = s.expires_at {
                            span { "{SUB_ACTIVE_UNTIL_LABEL} {fmt_local(exp)}" }
                        }
                    }
                }
                div { class: "pay-total", "{price_label}" }
                div { class: "row",
                    button {
                        class: if period() == BillingPeriodApi::Monthly { "btn ghost plan active" } else { "btn ghost plan" },
                        "aria-label": "plan-monthly",
                        onclick: move |_| period.set(BillingPeriodApi::Monthly),
                        "{SUB_PLAN_MONTHLY_LABEL}"
                    }
                    button {
                        class: if period() == BillingPeriodApi::Yearly { "btn ghost plan active" } else { "btn ghost plan" },
                        "aria-label": "plan-yearly",
                        onclick: move |_| period.set(BillingPeriodApi::Yearly),
                        "{SUB_PLAN_YEARLY_LABEL}"
                    }
                }
                p { class: "dim", {SUB_PACKAGE_LABEL} }
                p { class: "dim small", {SUB_DELETE_POLICY} }
                button {
                    class: "btn primary full",
                    disabled: "{busy}",
                    "aria-label": "subscribe",
                    onclick: move |_| {
                        let fut = (start_order.clone())(renewing);
                        spawn(async move { fut.await; });
                    },
                    if renewing { "Renew" } else { "Subscribe {price_label}" }
                }
            }
            match pay() {
                PayState::Idle => rsx! {},
                PayState::Pending => {
                    if let Some(o) = order() {
                        let amount = o.amount_usdc.to_string();
                        rsx! {
                            PayPending {
                                recipient: o.recipient,
                                amount,
                                mint: o.mint,
                                sig_input,
                                on_verify: do_verify,
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }
                PayState::Confirming => rsx! {
                    div { class: "card pay",
                        div { class: "pay-status", span { class: "pay-dot" } span { {SUB_CONFIRMING_MSG} } }
                    }
                },
                PayState::Confirmed => rsx! {
                    div { class: "banner ok", {SUB_CONFIRMED_MSG} }
                },
                PayState::Failed => rsx! {
                    div { class: "card stack",
                        div { class: "error", "{pay_err}" }
                        button {
                            class: "btn ghost full",
                            onclick: move |_| pay.set(PayState::Pending),
                            "Try again"
                        }
                    }
                },
            }
        }
    }
}

fn parse_dec(s: &str) -> Option<rust_decimal::Decimal> {
    let t = s.trim();
    if t.is_empty() {
        return Some(rust_decimal::Decimal::ZERO);
    }
    t.parse().ok()
}

fn parse_i64(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return Some(0);
    }
    t.parse().ok()
}

fn name_matches(goods: &[pos_api::schema::GoodsResponse], query: &str) -> Vec<Uuid> {
    let q = query.to_lowercase();
    goods
        .iter()
        .filter(|g| g.name.to_lowercase().contains(&q))
        .map(|g| g.goods_id)
        .collect()
}

fn search_filtered(
    goods: &[pos_api::schema::GoodsResponse],
    query: &str,
) -> Vec<pos_api::schema::GoodsResponse> {
    if query.trim().is_empty() {
        return goods.to_vec();
    }
    let ids = name_matches(goods, query);
    goods
        .iter()
        .filter(|g| ids.contains(&g.goods_id))
        .cloned()
        .collect()
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n * 2 {
        return String::from(s);
    }
    format!("{}\u{2026}{}", &s[..n], &s[s.len() - n..])
}

fn token_label(t: pos_api::schema::SplToken) -> &'static str {
    match t {
        pos_api::schema::SplToken::Usdc => "USDC",
        pos_api::schema::SplToken::Usdt => "USDT",
    }
}

fn dest_label(d: &pos_api::schema::DestinationResponse) -> String {
    if d.chain == pos_api::schema::Chain::Solana {
        format!("{} \u{00b7} {}", chain_label(d.chain), token_label(d.token))
    } else {
        chain_label(d.chain).to_owned()
    }
}

fn chain_label(chain: pos_api::schema::Chain) -> &'static str {
    match chain {
        pos_api::schema::Chain::Solana => "Solana",
        pos_api::schema::Chain::Bank => "PromptPay",
    }
}

#[derive(Clone, Debug, serde::Serialize)]
struct GoodsRow(pos_api::schema::GoodsResponse);

impl PartialEq for GoodsRow {
    fn eq(&self, other: &Self) -> bool {
        self.0.goods_id == other.0.goods_id
    }
}
impl Eq for GoodsRow {}

impl FieldAccessible for GoodsRow {
    fn get_field(&self, field_name: &str) -> Option<String> {
        match field_name {
            "name" => Some(self.0.name.clone()),
            "price" => Some(self.0.price.to_string()),
            "cost" => Some(self.0.cost.to_string()),
            "units" => Some(self.0.units.to_string()),
            "barcode" => Some(self.0.barcode.clone().unwrap_or_default()),
            _ => None,
        }
    }
}

const GOODS_COLS: &[(&str, &str)] = &[
    ("name", "Name"),
    ("price", "Price"),
    ("cost", "Cost"),
    ("units", "Units"),
    ("barcode", "Barcode"),
];
const TABLE_PAGE_SIZE: usize = 10;

#[component]
fn GoodsTab(token: String, base: String, shop_id: Uuid, is_admin: bool) -> Element {
    let modal = use_modal();
    let version = use_signal(|| 0u32);
    let toast = use_toast();
    let mut goods = use_signal(Vec::<pos_api::schema::GoodsResponse>::new);
    let load_base = base.clone();
    let load_token = token.clone();
    let ver = version.read().to_owned();
    use_resource(use_reactive!(|(ver,)| {
        let client = FetchClient {
            base_url: load_base.clone(),
        };
        let token = load_token.clone();
        async move {
            let _ = ver;
            if let Ok(g) = client.list_goods(&token, shop_id).await {
                goods.set(g);
            }
        }
    }));

    let create_base = base.clone();
    let create_token = token.clone();
    let create_modal = modal.clone();
    let open_create = move |_| {
        let mut modal = create_modal.clone();
        modal.set_title("Create goods");
        modal.set_content(rsx! {
            GoodsForm {
                token: create_token.clone(),
                base: create_base.clone(),
                shop_id,
                initial: None,
                on_done: version,
            }
        });
        modal.open();
    };

    let edit_token = token.clone();
    let edit_base = base.clone();
    let edit_modal = modal.clone();
    let del_base = base.clone();
    let del_token = token.clone();
    let del_toast = toast.clone();
    rsx! {
    div { class: "stack",
        div { class: "row split",
            h2 { class: "page-title", "Goods" }
            button {
                class: "btn primary",
                "aria-label": "create-goods",
                onclick: open_create,
                "+ Create"
            }
        }
        GoodsTable {
            goods,
            is_admin,
            on_edit: move |g: pos_api::schema::GoodsResponse| {
                let mut modal = edit_modal.clone();
                modal.set_title("Edit goods");
                modal.set_content(rsx! {
                    GoodsForm {
                        token: edit_token.clone(),
                        base: edit_base.clone(),
                        shop_id,
                        initial: Some(g),
                        on_done: version,
                    }
                });
                modal.open();
            },
            on_delete: move |g: pos_api::schema::GoodsResponse| {
                let client = FetchClient { base_url: del_base.clone() };
                let mut toast = del_toast.clone();
                let mut version = version;
                let goods_id = g.goods_id;
                let token = del_token.clone();
                spawn(async move {
                    match client.delete_goods(&token, shop_id, goods_id).await {
                        Ok(()) => version += 1,
                        Err(e) => notify(&mut toast, &e, true),
                    }
                });
            },
        }
    }
    }
}

fn sorted_sell_goods(
    goods: &[pos_api::schema::GoodsResponse],
    sales: &[pos_api::schema::GoodsSalesResponse],
    sort: GoodsSort,
) -> Vec<pos_api::schema::GoodsResponse> {
    let mut list = goods.to_vec();
    match sort {
        GoodsSort::Best => {
            let sold = |id: Uuid| {
                sales
                    .iter()
                    .find(|s| s.goods_id == id)
                    .map(|s| s.sold)
                    .unwrap_or(0)
            };
            list.sort_by_key(|g| std::cmp::Reverse(sold(g.goods_id)));
        }
        GoodsSort::Price => list.sort_by_key(|g| std::cmp::Reverse(g.price)),
        GoodsSort::Name => list.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    list
}

fn derive_rows(
    goods: &[pos_api::schema::GoodsResponse],
    sort_col: &str,
    desc: bool,
) -> Vec<GoodsRow> {
    let mut rows: Vec<GoodsRow> = goods.iter().cloned().map(GoodsRow).collect();
    rows.sort_by(|a, b| {
        let ord = a
            .get_field(sort_col)
            .unwrap_or_default()
            .cmp(&b.get_field(sort_col).unwrap_or_default());
        if desc { ord.reverse() } else { ord }
    });
    rows
}

fn slice_rows(rows: Vec<GoodsRow>, start: usize, end: usize) -> (PropData<GoodsRow>, usize) {
    let total = rows.len();
    let data_vec: Vec<GoodsRow> = rows
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    (PropData { data_vec }, total)
}

#[component]
fn GoodsTable(
    goods: ReadSignal<Vec<pos_api::schema::GoodsResponse>>,
    is_admin: bool,
    on_edit: EventHandler<pos_api::schema::GoodsResponse>,
    on_delete: EventHandler<pos_api::schema::GoodsResponse>,
) -> Element {
    let fetch = move |start: usize, end: usize, (sort_col, desc): (String, bool)| {
        let rows = derive_rows(&goods.read(), &sort_col, desc);
        Box::pin(async move { slice_rows(rows, start, end) })
            as Pin<Box<dyn Future<Output = (PropData<GoodsRow>, usize)>>>
    };
    let cols = PropCol {
        cols: GOODS_COLS
            .iter()
            .map(|(index, head)| Col {
                head: head.to_string(),
                index: index.to_string(),
                class: None,
                action: None,
            })
            .collect(),
    };
    let init_sort = ls_get(TABLE_SORT_KEY).and_then(|s| {
        s.split_once(SORT_STORE_SEP)
            .map(|(c, d)| (c.to_string(), d.to_string()))
    });
    let table = use_table(
        fetch,
        cols,
        Some(SortState {
            column: init_sort
                .as_ref()
                .map(|(c, _)| c.clone())
                .or(Some(String::from("name"))),
            descending: init_sort.is_some_and(|(_, d)| d == "1"),
        }),
    );

    // sync rows when the goods signal changes (create elsewhere); peek page/sort
    // so the signals written here are not effect deps (would retrigger forever)
    let effect_table = table.clone();
    use_effect(use_reactive!(|(goods,)| {
        let mut table = effect_table.clone();
        let page = table.page_state.peek().clone();
        let sort = table.sort_state.peek().clone();
        let rows = derive_rows(
            &goods.read(),
            &sort.column.unwrap_or_default(),
            sort.descending,
        );
        let (data, total) = slice_rows(
            rows,
            page.current_page * page.items_per_page,
            (page.current_page + 1) * page.items_per_page,
        );
        table.prop_data.set(data);
        table.page_state.write().total_items = total;
    }));

    rsx! {
        div { class: "card table-card",
            table { class: "rtable",
                thead {
                    tr {
                        for col in table.get_cols() {
                            th {
                                class: "sortable",
                                onclick: {
                                    let table = table.clone();
                                    let index = col.index.clone();
                                    move |_| {
                                        let mut t = table.clone();
                                        t.sort_by_field(&index);
                                        let col = t.get_sort_col();
                                        let val = format!(
                                            "{col}{}{}",
                                            SORT_STORE_SEP,
                                            u8::from(t.get_sort_state())
                                        );
                                        ls_set(TABLE_SORT_KEY, &val);
                                    }
                                },
                                "{col.head}"
                                if table.get_sort_col() == col.index {
                                    span { class: "sort-arrow", if table.get_sort_state() { "\u{25bc}" } else { "\u{25b2}" } }
                                }
                            }
                        }
                        if is_admin {
                            th { class: "actions-head", "" }
                        }
                    }
                }
                tbody {
                    for row in table.get_rows() {
                        tr {
                            for col in table.get_cols() {
                                td { class: if col.index == "price" { "num" } else { "" },
                                    {cell(&row, &col.index)}
                                }
                            }
                            if is_admin {
                                td { class: "actions-cell",
                                    button {
                                        class: "btn ghost tiny",
                                        "aria-label": format!("edit-goods-{}", row.0.goods_id),
                                        onclick: {
                                            let g = row.0.clone();
                                            move |_| on_edit.call(g.clone())
                                        },
                                        "{GOODS_EDIT_LABEL}"
                                    }
                                    button {
                                        class: "btn ghost tiny danger",
                                        "aria-label": format!("delete-goods-{}", row.0.goods_id),
                                        onclick: {
                                            let g = row.0.clone();
                                            move |_| on_delete.call(g.clone())
                                        },
                                        "{GOODS_DELETE_LABEL}"
                                    }
                                }
                            }
                        }
                    }
                    if table.get_rows().is_empty() {
                        tr { td { class: "empty", colspan: "{GOODS_COLS.len() + usize::from(is_admin)}", "no goods yet" } }
                    }
                }
            }
            TablePager { table }
        }
    }
}

fn cell(row: &GoodsRow, index: &str) -> Element {
    match index {
        "units" => {
            let units = row.0.units;
            if units > 0 {
                rsx! { span { class: if units <= LOW_STOCK { "unit-chip warn" } else { "unit-chip" }, "x{units}" } }
            } else {
                rsx! { span { class: "dim small", "—" } }
            }
        }
        "price" => rsx! { span { class: "goods-price", "{row.0.price}" } },
        "name" => rsx! {
            div { class: "goods-name-cell",
                {goods_icon_of(&row.0)}
                span { class: "goods-name", "{row.0.name}" }
                span { class: "chip", "{goods_type_label(&row.0.goods_type)}" }
            }
        },
        _ => rsx! { "{row.get_field(index).unwrap_or_default()}" },
    }
}

#[component]
fn TablePager(table: UseTable<GoodsRow>) -> Element {
    let page = table.get_page_state();
    let total_pages = (page.total_items + TABLE_PAGE_SIZE - 1) / TABLE_PAGE_SIZE.max(1);
    if total_pages <= 1 {
        return rsx! {};
    }
    let current = page.current_page + 1;
    let prev_table = table.clone();
    let next_table = table;
    rsx! {
        div { class: "pager",
            button {
                class: "btn ghost pager-btn",
                disabled: "{page.current_page == 0}",
                onclick: move |_| prev_table.clone().set_page(page.current_page.saturating_sub(1)),
                "\u{2039}"
            }
            span { class: "dim small", "{current} / {total_pages}" }
            button {
                class: "btn ghost pager-btn",
                disabled: "{page.current_page + 1 >= total_pages}",
                onclick: move |_| next_table.clone().set_page(page.current_page + 1),
                "\u{203a}"
            }
        }
    }
}

#[component]
fn GoodsForm(
    token: String,
    base: String,
    shop_id: Uuid,
    initial: Option<pos_api::schema::GoodsResponse>,
    on_done: Signal<u32>,
) -> Element {
    let modal = use_modal();
    let toast = use_toast();
    let empty = String::new();
    let (
        init_name,
        init_price,
        init_cost,
        init_units,
        init_barcode,
        init_icon,
        init_color,
        init_type,
    ) = match &initial {
        Some(g) => (
            g.name.clone(),
            g.price.normalize().to_string(),
            g.cost.normalize().to_string(),
            g.units.to_string(),
            g.barcode.clone().unwrap_or_default(),
            g.icon.clone(),
            g.color.clone(),
            g.goods_type.clone(),
        ),
        None => (
            empty.clone(),
            empty.clone(),
            empty.clone(),
            String::from("0"),
            empty,
            String::from(DEFAULT_ICON_KEY),
            String::from(DEFAULT_COLOR_KEY),
            String::from(DEFAULT_GOODS_TYPE),
        ),
    };
    let mut name = use_signal(move || init_name.clone());
    let mut price = use_signal(move || init_price.clone());
    let mut cost = use_signal(move || init_cost.clone());
    let mut units = use_signal(move || init_units.clone());
    let mut barcode = use_signal(move || init_barcode.clone());
    let mut icon = use_signal(move || init_icon.clone());
    let mut color = use_signal(move || init_color.clone());
    let mut goods_type = use_signal(move || init_type.clone());
    let mut barcode_format = use_signal(|| String::from(BARCODE_FMT_AUTO));
    let mut camera_on = use_signal(|| false);
    let busy = use_signal(|| false);
    let code = barcode().trim().to_string();
    let barcode_state = if code.is_empty() {
        None
    } else {
        Some(match barcode_format_parse(&barcode_format()) {
            Some(f) => pos_barcode::format::validate_as(f, &code),
            None => pos_barcode::format::validate(&code),
        })
    };
    let barcode_ok = barcode_state.as_ref().is_some_and(|res| res.is_ok());
    let submit_toast = toast.clone();
    let form_toast = toast.clone();
    let barcode_msg = barcode_state.as_ref().map(|res| match res {
        Ok(f) => format!("{BARCODE_OK_PREFIX}{f:?}"),
        Err(e) => e.to_string(),
    });
    rsx! {
        div { class: "stack",
            div { class: "field-group",
                span { class: "field-label", "Name" }
                input {
                    class: "field",
                    placeholder: "name",
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                }
            }
            div { class: "field-group",
                span { class: "field-label", "Type" }
                select {
                    class: "field",
                    "aria-label": "goods-type",
                    value: "{goods_type}",
                    onchange: move |e| goods_type.set(e.value()),
                    for (key, label) in GOODS_TYPE_CHOICES {
                        option { value: "{key}", "{label}" }
                    }
                }
            }
            div { class: "field-group",
                span { class: "field-label", "{FIELD_LABEL_PRICE}" }
                input {
                    class: "field",
                    inputmode: "decimal",
                    placeholder: "price",
                    value: "{price}",
                    oninput: move |e| price.set(e.value()),
                }
            }
            div { class: "field-group",
                span { class: "field-label", "{FIELD_LABEL_COST}" }
                input {
                    class: "field",
                    inputmode: "decimal",
                    placeholder: "cost",
                    value: "{cost}",
                    oninput: move |e| cost.set(e.value()),
                }
            }
            div { class: "field-group",
                span { class: "field-label", "{FIELD_LABEL_UNITS}" }
                input {
                    class: "field",
                    inputmode: "numeric",
                    placeholder: "units",
                    value: "{units}",
                    oninput: move |e| units.set(e.value()),
                }
                p { class: "dim small", "{UNITS_HINT}" }
            }
            div { class: "field-group",
                span { class: "field-label", "{FIELD_LABEL_BARCODE}" }
                div { class: "row",
                    input {
                        class: "field grow",
                        placeholder: "barcode (optional)",
                        value: "{barcode}",
                        oninput: move |e| barcode.set(e.value()),
                    }
                    button {
                        class: if camera_on() { "scan-toggle on" } else { "scan-toggle" },
                        "aria-label": "scan-barcode",
                        onclick: move |_| {
                            let t = form_toast.clone();
                            if camera_on() {
                                camera_on.set(false);
                                return;
                            }
                            if !camera_supported() {
                                let mut tc = t.clone();
                                notify(&mut tc, CAMERA_UNSUPPORTED_MSG, true);
                                return;
                            }
                            camera_on.set(true);
                            spawn(run_barcode_capture(camera_on, barcode, t));
                        },
                        if camera_on() { "{CAMERA_STOP_LABEL}" } else { "{BARCODE_SCAN_START_LABEL}" }
                    }
                }
                select {
                    class: "field",
                    "aria-label": "barcode-format",
                    value: "{barcode_format}",
                    onchange: move |e| barcode_format.set(e.value()),
                    for (key, label) in BARCODE_FORMAT_CHOICES {
                        option { value: "{key}", "{label}" }
                    }
                }
                if let Some(msg) = barcode_msg {
                    span {
                        class: if barcode_ok { "barcode-status ok" } else { "barcode-status err" },
                        "{msg}"
                    }
                }
                if camera_on() {
                    div { class: "cam-box",
                        video {
                            id: BARCODE_VIDEO_ID,
                            class: "cam-video",
                            autoplay: "true",
                            playsinline: "true",
                            muted: "true",
                        }
                    }
                }
            }
            div { class: "field-group",
                span { class: "field-label", "{FIELD_LABEL_ICON}" }
                div { class: "row",
                    GoodsIcon { icon: icon(), color: color(), size: 24 }
                    select {
                        class: "field grow",
                        "aria-label": "goods-icon",
                        value: "{icon}",
                        onchange: move |e| icon.set(e.value()),
                        for (key, label) in GOODS_ICON_CHOICES {
                            option { value: "{key}", "{label}" }
                        }
                    }
                }
            }
            div { class: "field-group",
                span { class: "field-label", "{FIELD_LABEL_COLOR}" }
                select {
                    class: "field",
                    "aria-label": "goods-color",
                    value: "{color}",
                    onchange: move |e| color.set(e.value()),
                    for (key, label) in GOODS_COLOR_CHOICES {
                        option { value: "{key}", "{label}" }
                    }
                }
            }
            button {
                class: "btn primary full",
                "aria-label": "add-goods",
                disabled: busy(),
                onclick: move |_| {
                    let goods_name = name().trim().to_string();
                    if goods_name.is_empty() {
                        notify(&mut submit_toast.clone(), GOODS_NAME_REQUIRED_MSG, true);
                        return;
                    }
                    if let Some(Err(e)) = &barcode_state {
                        notify(&mut submit_toast.clone(), &e.to_string(), true);
                        return;
                    }
                    let (Some(price_v), Some(cost_v), Some(units_v)) = (
                        parse_dec(&price()),
                        parse_dec(&cost()),
                        parse_i64(&units()),
                    ) else {
                        notify(&mut toast.clone(), GOODS_NUMBER_INVALID_MSG, true);
                        return;
                    };
                    let client = FetchClient { base_url: base.clone() };
                    let common = (
                        goods_name,
                        price_v,
                        cost_v,
                        units_v,
                        {
                            let b = barcode().trim().to_string();
                            if b.is_empty() { None } else { Some(b) }
                        },
                        icon().clone(),
                        color().clone(),
                        goods_type().clone(),
                    );
                    let token = token.clone();
                    let mut toast = toast.clone();
                    let mut modal = modal.clone();
                    let mut busy = busy;
                    let initial = initial.clone();
                    spawn(async move {
                        busy.set(true);
                        let (goods_name, price_v, cost_v, units_v, barcode, icon, color, goods_type) =
                            common;
                        let res = match &initial {
                            Some(g) => {
                                client
                                    .update_goods(
                                        &token,
                                        shop_id,
                                        g.goods_id,
                                        &UpdateGoodsRequest {
                                            name: goods_name,
                                            price: price_v,
                                            cost: cost_v,
                                            units: units_v,
                                            barcode,
                                            icon,
                                            color,
                                            goods_type,
                                        },
                                    )
                                    .await
                                    .map(|_| ())
                            }
                            None => {
                                client
                                    .add_goods(
                                        &token,
                                        &AddGoodsRequest {
                                            shop_id,
                                            name: goods_name,
                                            price: price_v,
                                            cost: cost_v,
                                            units: units_v,
                                            barcode,
                                            icon,
                                            color,
                                            goods_type,
                                        },
                                    )
                                    .await
                                    .map(|_| ())
                            }
                        };
                        match res {
                            Ok(()) => {
                                toast.success(if initial.is_some() { "goods updated" } else { "goods created" });
                                on_done.with_mut(|v| *v = v.wrapping_add(1));
                                modal.close();
                            }
                            Err(e) => toast.error(&e),
                        }
                        busy.set(false);
                    });
                },
                if busy() { "…" } else if initial.is_some() { "Save changes" } else { "Add goods" }
            }
        }
    }
}

struct PaymentSettings {
    shop_id: Uuid,
    mode: VatMode,
    vat_rate: String,
    fx_input: String,
    cur: Currency,
}

fn save_settings(token: String, base: String, toast: UseToast, s: PaymentSettings) {
    spawn(async move {
        let mut toast = toast;
        let client = FetchClient { base_url: base };
        let req = UpdateShopSettingsRequest {
            vat_mode: s.mode,
            vat_rate: s
                .vat_rate
                .parse::<rust_decimal::Decimal>()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            fx_thb_per_usd: s
                .fx_input
                .parse::<rust_decimal::Decimal>()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            currency: s.cur,
        };
        match client.update_shop_settings(&token, s.shop_id, &req).await {
            Ok(_) => notify(&mut toast, "Payment settings saved", false),
            Err(e) => notify(&mut toast, &e, true),
        }
    });
}

#[component]
fn SettingsTab(token: String, base: String, shop_id: Uuid, is_admin: bool) -> Element {
    let mut destinations = use_signal(Vec::<pos_api::schema::DestinationResponse>::new);
    let mut dest_chain = use_signal(|| Chain::Solana);
    let dest_token = use_signal(SplToken::default);
    let mut dest_address = use_signal(String::new);
    let wallet = use_wallet();
    let wallet_busy = wallet.busy.cloned();
    let toast = use_toast();
    let mut group = use_signal(|| SettingsGroup::Payments);
    let mut section = use_signal(|| SettingsSection::Receive);
    let mut vat_mode = use_signal(|| VatMode::None);
    let mut vat_rate = use_signal(String::new);
    let mut fx_input = use_signal(String::new);
    let mut currency = use_signal(|| Currency::Usdt);

    let base_future = base.clone();
    let token_future = token.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: base_future.clone(),
        };
        let token = token_future.clone();
        async move {
            if let Ok(d) = client.list_destinations(&token, shop_id).await {
                destinations.set(d);
            }
            if let Some(s) = client
                .list_shops(&token)
                .await
                .ok()
                .and_then(|shops| shops.into_iter().find(|s| s.shop_id == shop_id))
            {
                vat_mode.set(s.vat_mode);
                vat_rate.set(s.vat_rate.normalize().to_string());
                fx_input.set(s.fx_thb_per_usd.normalize().to_string());
                currency.set(s.currency);
            }
        }
    });

    rsx! {
        div { class: "stack",
            h2 { class: "page-title", "Settings" }

            div { class: "settings-tabs",
                for g in [SettingsGroup::Subscription, SettingsGroup::Payments, SettingsGroup::Usage] {
                    button {
                        class: if group() == g { "settings-tab active" } else { "settings-tab" },
                        "aria-label": g.id(),
                        onclick: move |_| {
                            group.set(g);
                            section.set(g.default_section());
                        },
                        "{g.label()}"
                    }
                }
            }

            div { class: "settings-split",
                nav { class: "settings-nav",
                    for s in group().sections().iter().copied() {
                        button {
                            class: if section() == s { "settings-item active" } else { "settings-item" },
                            "aria-label": s.id(),
                            onclick: move |_| section.set(s),
                            "{s.label()}"
                        }
                    }
                }

                div { class: "settings-body",
                    match section() {
                        SettingsSection::Renew => rsx! {
                            SubscriptionView { token: token.clone(), base: base.clone(), on_confirmed: move |_| {} }
                        },
                        SettingsSection::Usage => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Usage" }
                                QuotaUsage { token: token.clone(), base: base.clone() }
                            }
                        },
                        SettingsSection::Storage => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "{STORAGE_TITLE}" }
                                StorageView { token: token.clone(), base: base.clone(), shop_id, is_admin }
                            }
                        },
                        SettingsSection::Receive => rsx! {
                            div { class: "card form",
                                div { class: "row split",
                                    h3 { class: "doc-title", "Receive accounts" }
                                    if is_admin {
                                        button {
                                            class: "btn-circle",
                                            "aria-label": "add-account",
                                            title: "Add receive account",
                                            onclick: move |_| section.set(SettingsSection::AddAccount),
                                            "+"
                                        }
                                    }
                                }
                                div { class: "list",
                                    for d in destinations().iter().cloned() {
                                        div { class: "list-row",
                                            div { class: "goods-col",
                                                span { class: "chain", "{chain_label(d.chain)}" }
                                                span { class: "mono dim small", "{truncate(&d.address, 10)}" }
                                            }
                                            span { class: "chip", "receives here" }
                                            if is_admin {
                                                button {
                                                    class: "btn ghost tiny danger",
                                                    "aria-label": "delete-destination-{d.destination_id}",
                                                    onclick: {
                                                        let base = base.clone();
                                                        let token = token.clone();
                                                        let mut destinations = destinations;
                                                        let toast = toast.clone();
                                                        move |_| {
                                                            let client = FetchClient { base_url: base.clone() };
                                                            let token = token.clone();
                                                            let mut toast = toast.clone();
                                                            let dest_id = d.destination_id;
                                                            spawn(async move {
                                                                match client.remove_destination(&token, shop_id, dest_id).await {
                                                                    Ok(()) => {
                                                                        if let Ok(list) = client.list_destinations(&token, shop_id).await {
                                                                            destinations.set(list);
                                                                        }
                                                                    }
                                                                    Err(e) => notify(&mut toast, &e, true),
                                                                }
                                                            });
                                                        }
                                                    },
                                                    "{DEST_DELETE_LABEL}"
                                                }
                                            }
                                        }
                                    }
                                    if destinations().is_empty() {
                                        p { class: "dim pad", "no receive accounts yet — tap + to add one" }
                                    }
                                }
                            }
                        },
                        SettingsSection::AddAccount => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Add receive account" }
                                if dest_chain() == Chain::Solana {
                                    div { class: "row",
                                        for kind in WalletKind::ALL.into_iter().filter(|k| k.detect().is_some()) {
                                            button {
                                                class: "btn ghost",
                                                "aria-label": "connect-wallet",
                                                disabled: "{wallet_busy}",
                                                onclick: {
                                                    let base = base.clone();
                                                    let token = token.clone();
                                                    let wallet = wallet.clone();
                                                    let toast = toast.clone();
                                                    move |_| {
                                                        let client = FetchClient { base_url: base.clone() };
                                                        let token = token.clone();
                                                        let mut wallet = wallet.clone();
                                                        let mut toast = toast.clone();
                                                        spawn(async move {
                                                            wallet.connect(kind).await;
                                                            if let Some(e) = wallet.error.cloned() {
                                                                notify(&mut toast, &e, true);
                                                                return;
                                                            }
                                                            let Some(acct) = wallet.account.cloned() else { return; };
                                                            let req = AddDestinationRequest { shop_id, chain: Chain::Solana, address: acct.pubkey, token: SplToken::Usdc, label: Some(kind.name().to_string()) };
                                                            match client.add_destination(&token, &req).await {
                                                                Ok(_) => {
                                                                    if let Ok(d) = client.list_destinations(&token, shop_id).await { destinations.set(d); }
                                                                    section.set(SettingsSection::Receive);
                                                                }
                                                                Err(e) => notify(&mut toast, &e, true),
                                                            }
                                                        });
                                                    }
                                                },
                                                "Connect {kind.name()}"
                                            }
                                        }
                                    }
                                }
                                select {
                                    class: "field",
                                    "aria-label": "dest-chain",
                                    value: "{dest_chain:?}",
                                    onchange: move |e| {
                                        dest_chain.set(match e.value().as_str() {
                                            "Bank" => Chain::Bank,
                                            _ => Chain::Solana,
                                        });
                                    },
                                    option { value: "Solana", "Solana (USDC / USDT)" }
                                    option { value: "Bank", "PromptPay (Thai bank)" }
                                }
                                {(dest_chain() == Chain::Solana).then(|| {
                                    let mut dest_token = dest_token;
                                    rsx! {
                                        select {
                                            class: "field",
                                            "aria-label": "spl-token",
                                            value: "{dest_token:?}",
                                            onchange: move |e| {
                                                dest_token.set(match e.value().as_str() {
                                                    "Usdc" => SplToken::Usdc,
                                                    "Usdt" => SplToken::Usdt,
                                                    _ => SplToken::Usdc,
                                                });
                                            },
                                            option { value: "Usdc", "Pay in USDC" }
                                            option { value: "Usdt", "Pay in USDT" }
                                        }
                                    }
                                })}
                                div { class: "row",
                                    input {
                                        class: "field grow",
                                        placeholder: "address",
                                        value: "{dest_address}",
                                        oninput: move |e| dest_address.set(e.value()),
                                    }
                                    button {
                                        class: "btn primary",
                                        "aria-label": "add-destination",
                                        onclick: {
                                            let base = base.clone();
                                            let token = token.clone();
                                            let mut destinations = destinations;
                                            let mut dest_address = dest_address;
                                            let toast = toast.clone();
                                            let mut section = section;
                                            move |_| {
                                                let client = FetchClient { base_url: base.clone() };
                                                let token = token.clone();
                                                let mut toast = toast.clone();
                                                let chain = dest_chain();
                                                let address = dest_address().clone();
                                                spawn(async move {
                                                    match client.add_destination(&token, &AddDestinationRequest { shop_id, chain, address, token: dest_token(), label: None }).await {
                                                        Ok(_) => {
                                                            dest_address.set(String::new());
                                                            if let Ok(d) = client.list_destinations(&token, shop_id).await { destinations.set(d); }
                                                            section.set(SettingsSection::Receive);
                                                        }
                                                        Err(e) => notify(&mut toast, &e, true),
                                                    }
                                                });
                                            }
                                        },
                                        "+"
                                    }
                                }
                                p { class: "dim small", "wallet address · PromptPay phone / national ID for Thai bank — shown as QR on every sale" }
                            }
                        },
                        SettingsSection::Tax => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Tax / VAT" }
                                label { class: "dim small", "VAT mode" }
                                select {
                                    class: "field",
                                    "aria-label": "vat-mode",
                                    value: "{vat_mode:?}",
                                    onchange: move |e| {
                                        vat_mode.set(match e.value().as_str() {
                                            "Exclusive" => VatMode::Exclusive,
                                            "Inclusive" => VatMode::Inclusive,
                                            _ => VatMode::None,
                                        });
                                    },
                                    option { value: "None", "No VAT — don't charge VAT" }
                                    option { value: "Exclusive", "Exclusive — add VAT on top of prices" }
                                    option { value: "Inclusive", "Inclusive — prices already include VAT" }
                                }
                                if vat_mode() != VatMode::None {
                                    label { class: "dim small", "VAT rate (%)" }
                                    input {
                                        class: "field",
                                        "aria-label": "vat-rate",
                                        placeholder: "VAT %",
                                        value: "{vat_rate}",
                                        oninput: move |e| vat_rate.set(e.value()),
                                    }
                                }
                                button {
                                    class: "btn primary full",
                                    "aria-label": "save-vat",
                                    onclick: move |_| save_settings(token.clone(), base.clone(), toast.clone(), PaymentSettings { shop_id, mode: vat_mode(), vat_rate: vat_rate(), fx_input: fx_input(), cur: currency() }),
                                    "Save VAT settings"
                                }
                                p { class: "dim small", "exclusive adds VAT on top of prices · inclusive extracts it — shown on every sale" }
                            }
                        },
                        SettingsSection::Currency => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Currency & FX" }
                                label { class: "dim small", "Display currency" }
                                select {
                                    class: "field",
                                    "aria-label": "currency",
                                    value: "{currency:?}",
                                    onchange: move |e| {
                                        currency.set(match e.value().as_str() {
                                            "Usdt" => Currency::Usdt,
                                            _ => Currency::Thb,
                                        });
                                    },
                                    option { value: "Usdt", "USD $ — USDC / USDT" }
                                    option { value: "Thb", "THB \u{0e3f} — Thai baht" }
                                }
                                label { class: "dim small", "THB per $1 (swap rate)" }
                                input {
                                    class: "field",
                                    "aria-label": "fx-thb-per-usd",
                                    placeholder: "THB per $1 (0 = off)",
                                    value: "{fx_input}",
                                    oninput: move |e| fx_input.set(e.value()),
                                }
                                button {
                                    class: "btn primary full",
                                    "aria-label": "save-currency",
                                    onclick: move |_| save_settings(token.clone(), base.clone(), toast.clone(), PaymentSettings { shop_id, mode: vat_mode(), vat_rate: vat_rate(), fx_input: fx_input(), cur: currency() }),
                                    "Save currency settings"
                                }
                                p { class: "dim small", "prices are charged in this currency · FX rate converts $ prices to THB · 0 disables conversion" }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn ReportsTab(token: String, base: String, shop_id: Uuid) -> Element {
    let mut billings = use_signal(Vec::<pos_api::schema::BillingResponse>::new);
    let mut cfg = use_signal(|| Option::<pos_api::schema::ShopResponse>::None);
    let toast = use_toast();

    let base_future = base.clone();
    let token_future = token.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: base_future.clone(),
        };
        let token = token_future.clone();
        let mut toast = toast.clone();
        async move {
            match client.list_billings(&token, shop_id).await {
                Ok(i) => billings.set(i),
                Err(e) => notify(&mut toast, &e, true),
            }
            if let Ok(shops) = client.list_shops(&token).await {
                cfg.set(shops.into_iter().find(|s| s.shop_id == shop_id));
            }
        }
    });

    let currency = cfg()
        .map(|s| s.currency)
        .unwrap_or(pos_api::schema::Currency::Usdt);
    let sym = if currency == pos_api::schema::Currency::Usdt {
        "$"
    } else {
        "\u{0e3f}"
    };
    let show = move |v: rust_decimal::Decimal| v.round_dp(2).normalize();

    let zero = rust_decimal::Decimal::ZERO;
    let revenue = billings().iter().map(|i| i.total).fold(zero, |a, t| a + t);
    let cost = billings().iter().map(|i| i.cost).fold(zero, |a, t| a + t);
    let profit = revenue - cost;
    let rows: Vec<(String, pos_api::schema::BillingResponse)> = billings()
        .iter()
        .cloned()
        .map(|inv| {
            let name = inv
                .lines
                .first()
                .map(|l| l.name.clone())
                .unwrap_or_else(String::new);
            (name, inv)
        })
        .collect();

    rsx! {
        div { class: "stack",
            h2 { class: "page-title", "Reports" }
            div { class: "stats",
                div { class: "card stat",
                    span { class: "stat-label", "Revenue" }
                    span { class: "stat-value", "{sym} {show(revenue)}" }
                    span { class: "dim small", "{billings().len()} sales" }
                }
                div { class: "card stat",
                    span { class: "stat-label", "Profit" }
                    span { class: "stat-value good", "{sym} {show(profit)}" }
                    span { class: "dim small", "after {sym}{show(cost)} cost" }
                }
            }
            h3 { class: "section-title", "Sales documents" }
            div { class: "list",
                for (name, inv) in rows.iter() {
                    div { class: "list-row",
                        div { class: "goods-col",
                            span { class: "goods-name", "{name}" }
                            span { class: "dim small", "{chain_label(inv.destination.chain)} \u{00b7} margin {sym}{show(inv.total - inv.cost)}" }
                        }
                        div { class: "goods-col right",
                            span { class: "goods-price", "{sym} {show(inv.total)}" }
                            a { class: "dim small pdf-link", href: "{base}{inv.pdf_url}", target: "_blank", "PDF" }
                        }
                    }
                }
                if billings().is_empty() {
                    p { class: "dim pad", "no sales yet — complete one in the Sell tab" }
                }
            }
        }
    }
}

const STYLE: &str = "* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { background: #221418; color: #fff3e9; font-family: system-ui, -apple-system, sans-serif; }
.app { min-height: 100vh; }
.dim { color: #c9a8a0; }
.small { font-size: 12px; }
.mono { font-family: ui-monospace, monospace; }
.pad { padding: 16px; }
.grow { flex: 1; }
.error { color: #ff8fa3; font-size: 13px; padding: 8px 16px; }
.success { color: #9fe6c8; font-size: 13px; padding: 8px 16px; }
.row { display: flex; gap: 8px; }
.stack { display: flex; flex-direction: column; gap: 16px; }

.auth { min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 24px; padding: 24px; }
.brand { text-align: center; }
.brand h1 { font-size: 42px; letter-spacing: 8px; color: #ffab73; text-shadow: 0 2px 0 #5c2e3a, 0 0 24px rgba(255, 171, 115, 0.35); }
.brand h1::after { content: \"\"; display: inline-block; width: 26px; height: 26px; margin-left: 10px; vertical-align: middle; background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"%23ffab73\" d=\"M4 4l4 2.5a8 8 0 0 1 8 0L20 4v9a8 8 0 0 1-16 0V4Z\"/><circle cx=\"9.2\" cy=\"11\" r=\"1.3\" fill=\"%233a1d28\"/><circle cx=\"14.8\" cy=\"11\" r=\"1.3\" fill=\"%233a1d28\"/></svg>') no-repeat center/contain; }
.brand::before { content: \"いらっしゃいませ\"; display: block; font-size: 13px; letter-spacing: 4px; color: #ffb3c6; margin-bottom: 14px; padding-left: 22px; background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><g fill=\"%23ffb3c6\"><circle cx=\"12\" cy=\"5\" r=\"3.2\"/><circle cx=\"18.5\" cy=\"9.8\" r=\"3.2\"/><circle cx=\"16\" cy=\"17.4\" r=\"3.2\"/><circle cx=\"8\" cy=\"17.4\" r=\"3.2\"/><circle cx=\"5.5\" cy=\"9.8\" r=\"3.2\"/></g><circle cx=\"12\" cy=\"12\" r=\"3\" fill=\"%233a1d28\"/></svg>') no-repeat left center/14px; }
.brand-mark { width: 72px; height: 72px; margin: 0 auto 12px; border-radius: 26px; background: linear-gradient(145deg, #ffab73, #ff8fb3); color: #3a1d28; display: flex; align-items: center; justify-content: center; box-shadow: 0 6px 0 #5c2e3a, 0 12px 28px rgba(255, 143, 179, 0.25); }
.form { display: flex; flex-direction: column; gap: 10px; width: 100%; max-width: 360px; }
.card { background: #321e26; border: 1px solid #452a35; border-radius: 20px; padding: 16px; }

.field { background: #2a181f; border: 1px solid #452a35; border-radius: 14px; color: #fff3e9; padding: 12px 14px; font-size: 15px; outline: none; width: 100%; }
.field:focus { border-color: #ffab73; box-shadow: 0 0 0 3px rgba(255, 171, 115, 0.2); }
select.field { appearance: none; }
.field-group { display: flex; flex-direction: column; gap: 6px; }
.field-label { font-size: 11px; text-transform: uppercase; letter-spacing: 2px; color: #c9a8a0; }
.barcode-status { font-size: 12px; }
.barcode-status.ok { color: #9fe6c8; }
.barcode-status.err { color: #ff8fa3; }

.btn { border: none; border-radius: 14px; padding: 12px 18px; font-size: 15px; font-weight: 700; cursor: pointer; color: #fff3e9; background: #452a35; transition: transform 0.12s ease; }
button:focus { outline: none; }
button:focus-visible { outline: 2px solid #ffab73; outline-offset: 1px; }
.btn:disabled { opacity: 0.4; cursor: default; }
.btn.primary { background: linear-gradient(145deg, #ffab73, #ff8c5f); color: #3a1d28; box-shadow: 0 4px 0 #b05a3c; }
.btn.primary:not(:disabled):active { transform: translateY(2px); box-shadow: 0 2px 0 #b05a3c; }
.btn.ghost { background: transparent; border: 2px solid #ff9eb5; color: #ffb3c6; }
.btn.ghost.plan { flex: 1; }
.btn.tiny { padding: 6px 12px; font-size: 12px; border-radius: 10px; border-width: 1px; }
.btn.ghost.danger { border-color: #ff8fa3; color: #ff8fa3; }
.actions-cell { display: flex; gap: 6px; justify-content: flex-end; white-space: nowrap; }
.actions-head { width: 1%; }
.list-row .chip { flex: none; white-space: nowrap; }
.btn.ghost.plan.active { background: rgba(255, 158, 181, 0.18); border-color: #ffab73; color: #ffab73; }
.btn.full { width: 100%; }
.icon-btn { background: transparent; border: none; color: #c9a8a0; cursor: pointer; padding: 6px; display: flex; align-items: center; }
.icon-btn:hover { color: #ffab73; }

.shell { max-width: 480px; margin: 0 auto; min-height: 100vh; display: flex; flex-direction: column; }
.panel { display: flex; flex-direction: column; min-height: 100vh; }
.topbar { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; }
.brand-mini { color: #ffab73; display: flex; }
.content { flex: 1; padding: 8px 16px 96px; }

.tabbar { position: fixed; bottom: 18px; left: 50%; transform: translateX(-50%); display: flex; gap: 6px; background: #321e26; border: 1px solid #452a35; border-radius: 999px; padding: 8px 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.45), 0 0 0 4px rgba(255, 158, 181, 0.08); }
.tab { background: transparent; border: none; color: #c9a8a0; cursor: pointer; padding: 10px 18px; border-radius: 999px; display: flex; align-items: center; transition: transform 0.15s ease; }
.tab:hover { transform: translateY(-2px); }
.tab.active { background: linear-gradient(145deg, #ffab73, #ff8c5f); color: #3a1d28; animation: tab-pop 0.25s ease; }
@keyframes tab-pop { 0% { transform: scale(0.85); } 60% { transform: scale(1.08); } 100% { transform: scale(1); } }

.shop-list { display: flex; flex-wrap: wrap; gap: 8px; }
.shop-chip { background: #2a181f; border: 1px solid #452a35; color: #fff3e9; border-radius: 999px; padding: 8px 14px; cursor: pointer; font-size: 14px; }
.shop-chip.active { background: #4d2b3a; border-color: #ffab73; color: #ffab73; }

.scan-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.scan-toggle { background: #4d2b3a; border: 1px solid #ffab73; color: #ffab73; border-radius: 999px; padding: 8px 14px; cursor: pointer; font-size: 13px; }
.scan-toggle.on { background: #ffab73; color: #221418; }
.cam-box { border-radius: 18px; overflow: hidden; border: 1px solid #452a35; background: #000; }
.cam-video { width: 100%; max-height: 280px; object-fit: cover; display: block; }

.goods-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.goods-tile { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; background: #321e26; border: 1px solid #452a35; border-radius: 18px; padding: 14px; cursor: pointer; color: #fff3e9; text-align: left; transition: transform 0.12s ease, border-color 0.12s ease; }
.goods-tile:not(.soldout):hover { transform: translateY(-2px); border-color: #ff9eb5; }
.goods-tile.picked { border-color: #ffab73; background: #4d2b3a; box-shadow: 0 0 0 3px rgba(255, 171, 115, 0.18); }
.goods-name { font-size: 14px; font-weight: 600; }
.goods-price { color: #ffab73; font-weight: 700; font-size: 15px; }
.goods-meta { display: flex; align-items: center; gap: 8px; width: 100%; justify-content: space-between; }
.cart-list { gap: 6px; }
.cart-card { background: #321e26; border: 1px solid #452a35; border-radius: 18px; padding: 12px 14px 6px; display: flex; flex-direction: column; }
.cart-head { display: flex; align-items: center; gap: 10px; margin-bottom: 4px; }
.cart-head .spacer { flex: 1; }
.cart-clear { background: transparent; border: none; color: #ff8fa3; font-size: 12px; font-weight: 700; cursor: pointer; padding: 4px 8px; }
.cart-row { display: flex; justify-content: space-between; align-items: center; padding: 8px 0; border-top: 1px solid #3b2430; }
.cart-row .goods-name { font-weight: 700; }
.sell-stack { padding-bottom: 170px; }
.sort-select { width: auto; min-width: 130px; max-width: 190px; border-radius: 999px; padding: 8px 14px; }
.cart-qty { gap: 6px; align-items: center; }
.qty-btn { width: 28px; height: 28px; border-radius: 999px; border: 1px solid #452a35; background: #321e26; color: #ffab73; font-size: 16px; font-weight: 700; cursor: pointer; }
.qty-btn-lg { width: 52px; height: 52px; font-size: 26px; }
.qty-edit { display: flex; align-items: center; justify-content: center; gap: 22px; padding: 12px 0 20px; }
.qty-num-lg { font-size: 30px; font-weight: 800; min-width: 84px; text-align: center; }
.qty-btn:hover { border-color: #ffab73; }
.qty-num { min-width: 32px; text-align: center; font-weight: 700; }
.unit-chip { background: #452a35; color: #c9a8a0; border-radius: 999px; padding: 2px 8px; font-size: 11px; font-weight: 700; }
.goods-tile.picked .unit-chip { color: #ffab73; }

.page-title { font-size: 22px; font-weight: 800; letter-spacing: 0.5px; }
.page-title::before { content: \"\"; display: inline-block; width: 14px; height: 14px; margin-right: 8px; vertical-align: -1px; background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"%23ff9eb5\" d=\"M12 2l2.5 7.1L22 12l-7.5 2.9L12 22l-2.5-7.1L2 12l7.5-2.9L12 2Z\"/></svg>') no-repeat center/contain; }
.doc-title { font-size: 13px; text-transform: uppercase; letter-spacing: 2px; color: #c9a8a0; }

.list-row.goods { align-items: center; }
.goods-col { display: flex; flex-direction: column; gap: 3px; }
.goods-col.right { align-items: flex-end; }
.field.units { max-width: 80px; }

.dest-strip { display: flex; flex-wrap: wrap; gap: 8px; }
.dest-chip { background: #321e26; border: 1px solid #452a35; color: #c9a8a0; border-radius: 999px; padding: 8px 12px; cursor: pointer; font-size: 12px; font-family: ui-monospace, monospace; }
.dest-chip.active { border-color: #a8e6cf; color: #a8e6cf; background: #2c3a33; }

.goods-img { width: 44px; height: 44px; border-radius: 14px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
.goods-name-cell { display: flex; align-items: center; gap: 8px; }

.shop-select { max-width: 420px; margin: 16vh auto 0; }
.onboard { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; margin-bottom: 8px; }

.settings-tabs { display: flex; gap: 6px; border-bottom: 1px solid #452a35; padding-bottom: 10px; }
.settings-tab { background: transparent; border: none; color: #c9a8a0; cursor: pointer; padding: 8px 16px; border-radius: 999px; font-size: 14px; font-weight: 700; }
.settings-tab.active { background: #4d2b3a; color: #ffab73; }
.settings-split { display: grid; grid-template-columns: 150px 1fr; gap: 12px; align-items: start; }
.settings-nav { display: flex; flex-direction: column; gap: 6px; }
.settings-item { background: transparent; border: 1px solid transparent; color: #c9a8a0; cursor: pointer; padding: 10px 12px; border-radius: 14px; text-align: left; font-size: 13px; }
.settings-item.active { background: #321e26; border-color: #452a35; color: #fff3e9; }
.settings-body { min-width: 0; }
@media (max-width: 560px) { .settings-split { grid-template-columns: 1fr; } .settings-nav { flex-direction: row; flex-wrap: wrap; } }

.fab { position: fixed; bottom: 92px; right: 20px; width: 62px; height: 62px; border-radius: 50%; background: linear-gradient(145deg, #ffab73, #ff8c5f); color: #3a1d28; border: none; cursor: pointer; display: flex; align-items: center; justify-content: center; box-shadow: 0 6px 0 #b05a3c, 0 12px 24px rgba(0,0,0,0.4); z-index: 40; transition: transform 0.12s ease; }
.btn-circle { flex: none; width: 34px; height: 34px; border-radius: 50%; border: 1px solid #ff9eb5; background: #452a35; color: #ffab73; font-size: 20px; line-height: 1; cursor: pointer; display: flex; align-items: center; justify-content: center; }
.btn-circle:active { transform: scale(0.94); }
.fab:disabled { opacity: 0.4; cursor: default; }
.fab:active { transform: scale(0.94); }
.fab.hold { bottom: 240px; background: #452a35; color: #ffab73; border: 1px solid #ff9eb5; box-shadow: none; }
.fab.cart { bottom: 166px; background: #452a35; color: #ffab73; border: 1px solid #ff9eb5; box-shadow: none; }
.fab-badge { position: absolute; top: -4px; right: -2px; background: #ff8c5f; color: #3a1d28; border-radius: 999px; min-width: 22px; height: 22px; padding: 0 6px; font-size: 12px; font-weight: 800; display: flex; align-items: center; justify-content: center; }
.dest-chip.held-chip { color: #ffab73; border-color: #ff9eb5; }

.pay { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 24px; }
.pay-total { font-size: 28px; font-weight: 800; color: #ffab73; }
.qr { background: #fff8f0; padding: 12px; border-radius: 18px; box-shadow: 0 0 0 4px rgba(255, 158, 181, 0.25); animation: qr-pop 0.25s cubic-bezier(0.34, 1.56, 0.64, 1); transform-origin: center; }
@keyframes qr-pop { 0% { opacity: 0; transform: scale(0.85); } 100% { opacity: 1; transform: scale(1); } }
.qr svg { display: block; width: 220px; height: 220px; margin: 0 auto; }
.countdown { font-size: 20px; font-weight: 700; font-variant-numeric: tabular-nums; }
.countdown.urgent { color: #ff8fa3; animation: urgent-throb 1s ease-in-out infinite; }
@keyframes urgent-throb { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.55; transform: scale(1.06); } }
.expired { text-align: center; color: #ff8fa3; display: flex; flex-direction: column; gap: 12px; align-items: center; }

.list { display: flex; flex-direction: column; gap: 8px; }
.list-row { display: flex; align-items: center; justify-content: space-between; background: #321e26; border: 1px solid #452a35; border-radius: 16px; padding: 12px 14px; font-size: 14px; }
.list-row.clickable { cursor: pointer; }
.list-row.clickable:hover { border-color: #ffab73; }
.chip { background: #4d2b3a; color: #ffb3c6; border-radius: 999px; padding: 3px 10px; font-size: 11px; text-transform: uppercase; font-weight: 700; }
.chain { font-weight: 700; font-size: 12px; color: #cdb4f6; }
.field.price { max-width: 90px; }

.onboard { display: flex; flex-direction: column; gap: 6px; margin-bottom: 12px; }
.section-title { font-size: 13px; text-transform: uppercase; letter-spacing: 2px; color: #c9a8a0; margin-top: 8px; }
.stats { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.stat { display: flex; flex-direction: column; gap: 4px; }
.stat-label { font-size: 12px; text-transform: uppercase; letter-spacing: 2px; color: #c9a8a0; }
.stat-value { font-size: 26px; font-weight: 800; color: #ffab73; }
.stat-value.good { color: #9fe6c8; }
.unit-chip.warn { color: #ffe3a3; }
.goods-tile.soldout { opacity: 0.45; cursor: default; }
.pdf-link { text-decoration: none; }
a.pdf-link:hover { color: #ffab73; }

.total-bar { position: fixed; bottom: 92px; left: 20px; right: 96px; display: flex; justify-content: space-between; align-items: center; background: #321e26; border: 1px solid #452a35; border-radius: 999px; padding: 12px 20px; font-weight: 700; box-shadow: 0 8px 24px rgba(0,0,0,0.45), 0 0 0 4px rgba(168, 230, 207, 0.08); z-index: 39; }
.method-sheet { display: flex; flex-direction: column; gap: 10px; animation: sheet-up 0.2s ease-out; }
@keyframes sheet-up { 0% { opacity: 0; transform: translateY(14px); } 100% { opacity: 1; transform: translateY(0); } }
.method-tile { display: flex; align-items: center; justify-content: space-between; gap: 10px; background: #321e26; border: 1px solid #452a35; border-radius: 18px; padding: 18px 16px; font-size: 15px; cursor: pointer; transition: transform 0.15s ease, border-color 0.15s ease; }
.method-tile:hover { transform: scale(1.02); border-color: #ffab73; }
.method-tile:active { transform: scale(0.97); }
.method-tile .goods-price { font-size: 17px; font-weight: 800; color: #ffab73; }
.row.split { justify-content: space-between; align-items: center; }

.rtable { width: 100%; border-collapse: collapse; font-size: 14px; }
.rtable th { text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 1.5px; color: #c9a8a0; padding: 8px 10px; border-bottom: 1px solid #452a35; cursor: pointer; user-select: none; white-space: nowrap; }
.rtable th.sortable:hover { color: #ffab73; }
.sort-arrow { margin-left: 4px; color: #ffab73; }
.rtable td { padding: 10px; border-bottom: 1px solid #3a2330; }
.rtable tr:last-child td { border-bottom: none; }
.rtable td.num { text-align: right; font-family: ui-monospace, monospace; }
.rtable td.empty { text-align: center; color: #c9a8a0; padding: 20px; }
.table-card { padding: 8px; overflow-x: auto; }
.pager { display: flex; align-items: center; justify-content: center; gap: 12px; padding: 10px 0 4px; }
.pager-btn { padding: 6px 14px; }

@media (min-width: 768px) {
  .shell { max-width: 1040px; padding: 0 24px; }
  .goods-grid { grid-template-columns: repeat(4, 1fr); }
  .stats { grid-template-columns: repeat(4, 1fr); }
  .shop-list { flex-wrap: nowrap; }
  .total-bar { left: 50%; right: auto; transform: translateX(-50%); width: calc(min(100vw, 1040px) - 190px); }
  .fab { right: max(24px, calc(50% - 520px + 24px)); }
  .qr svg { width: 260px; height: 260px; margin: 0 auto; }
}

.pay-status { display: flex; align-items: center; justify-content: center; gap: 8px; font-size: 13px; color: #c9a8a0; }
.pay-dot { width: 8px; height: 8px; border-radius: 50%; background: #ffab73; animation: pay-pulse 1.6s ease-in-out infinite; }
.pay-dot.burst { animation: pay-burst 0.4s cubic-bezier(0.34, 1.56, 0.64, 1); background: #9fe6c8; }
@keyframes pay-burst { 0% { transform: scale(0.4); box-shadow: 0 0 0 0 rgba(159, 230, 200, 0.7); } 100% { transform: scale(1.6); box-shadow: 0 0 0 12px rgba(159, 230, 200, 0); } }
@keyframes pay-pulse { 0%, 100% { opacity: 0.25; } 50% { opacity: 1; } }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; transition-duration: 0.01ms !important; }
}
@media (max-width: 767px) {
  .fab { right: max(20px, calc(50% - 240px + 20px)); }
}

.toast-host { position: fixed; top: 16px; right: 16px; display: flex; flex-direction: column; align-items: flex-end; gap: 8px; z-index: 30; width: min(calc(100vw - 32px), 420px); pointer-events: none; }
.toast { pointer-events: none; display: flex; align-items: center; justify-content: space-between; gap: 12px; background: #321e26; border: 1px solid #452a35; border-left: 4px solid #c9a8a0; border-radius: 14px; padding: 10px 14px; font-size: 14px; box-shadow: 0 8px 24px rgba(0,0,0,0.45); }
.toast::before { content: \"\"; flex: none; width: 16px; height: 16px; margin-right: 4px; background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"%23c9a8a0\"/><circle cx=\"9\" cy=\"10\" r=\"1.3\" fill=\"%233a1d28\"/><circle cx=\"15\" cy=\"10\" r=\"1.3\" fill=\"%233a1d28\"/><path d=\"M8.5 14a4.5 4.5 0 0 0 7 0\" stroke=\"%233a1d28\" stroke-width=\"1.8\" fill=\"none\" stroke-linecap=\"round\"/></svg>') no-repeat center/contain; }
.toast .toast-close { pointer-events: auto; }
.toast.success { border-left-color: #9fe6c8; }
.toast.success::before { background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"%239fe6c8\"/><path d=\"M7.5 12.5l3 3 6-6.5\" stroke=\"%23274433\" stroke-width=\"2.4\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>') no-repeat center/contain; }
.toast.error { border-left-color: #ff8fa3; }
.toast.error::before { background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"%23ff8fa3\"/><path d=\"M8.5 8.5l7 7m0-7l-7 7\" stroke=\"%235c2e3a\" stroke-width=\"2.4\" stroke-linecap=\"round\"/></svg>') no-repeat center/contain; }
.toast.warning { border-left-color: #ffe3a3; }
.toast.warning::before { background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"%23ffe3a3\" d=\"M12 3l10 17.5H2L12 3Z\"/><path d=\"M12 10v4.5\" stroke=\"%235c4726\" stroke-width=\"2.2\" stroke-linecap=\"round\"/><circle cx=\"12\" cy=\"17.6\" r=\"1.3\" fill=\"%235c4726\"/></svg>') no-repeat center/contain; }
.toast.info { border-left-color: #ffab73; }
.toast.info::before { background: url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"%23ffab73\"/><path d=\"M12 11v5.5\" stroke=\"%235c2e3a\" stroke-width=\"2.2\" stroke-linecap=\"round\"/><circle cx=\"12\" cy=\"7.8\" r=\"1.3\" fill=\"%235c2e3a\"/></svg>') no-repeat center/contain; }
.toast-close { background: transparent; border: none; color: #c9a8a0; cursor: pointer; padding: 2px 4px; font-size: 12px; }
.toast-close:hover { color: #fff3e9; }

.modal-overlay { position: fixed; inset: 0; background: rgba(20, 8, 14, 0.75); display: flex; align-items: center; justify-content: center; padding: 20px; z-index: 50; }
.modal { position: relative; background: #321e26; border: 1px solid #452a35; border-radius: 24px; padding: 20px; width: 100%; max-width: 400px; max-height: calc(100vh - 40px); overflow-y: auto; display: flex; flex-direction: column; gap: 14px; box-shadow: 0 16px 48px rgba(0,0,0,0.55), 0 0 0 4px rgba(255, 158, 181, 0.1); }
.modal-close { position: absolute; top: 10px; right: 10px; font-size: 20px; }
.center { align-items: center; }

.banner { border-radius: 14px; padding: 10px 14px; font-size: 13px; margin-bottom: 8px; }
.banner.ok { background: #2c3a33; color: #9fe6c8; border: 1px solid #9fe6c8; }
.banner.warn { background: #3a3320; color: #ffe3a3; border: 1px solid #ffe3a3; }
.banner.err { background: #3a2028; color: #ff8fa3; border: 1px solid #ff8fa3; }
.banner.expiring { background: #3a2028; color: #ffb3c0; border: 2px solid #ff4d5e; }
.quota-row { display: flex; flex-direction: column; gap: 8px; margin-bottom: 8px; }
.quota { display: flex; flex-direction: column; gap: 3px; }
.bar { height: 8px; border-radius: 999px; background: #452a35; overflow: hidden; }
.bar-fill { height: 100%; border-radius: 999px; background: linear-gradient(90deg, #ffab73, #ff8fb3); }

";
