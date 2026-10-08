//! Dioxus wasm app root: login → shop POS (sell, goods, wallet, team) with QR pay.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use pos_api::schema::{
    AddDestinationRequest, AddGoodsRequest, Chain, CheckoutRequest, CreateShopRequest,
    InviteRequest, LoginRequest, RegisterRequest, Role, SplToken, UpdateShopSettingsRequest,
    VatMode,
};
use reslt_core::prelude::*;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

use crate::api_client::FetchClient;
use sol_wallet_adaptor::hooks::use_wallet;
use sol_wallet_adaptor::wallet::WalletKind;
use ui_interface::traits::ApiClient;

const DEFAULT_BASE: &str = "http://localhost:8080";
const BILLING_TITLE: &str = "Billing";
const QR_TTL_MS: i64 = 5 * 60 * 1000;
const TICK_MS: i32 = 1000;
const URGENCY_MS: i64 = 30_000;
const LOW_STOCK: i64 = 3;
const HELD_BILL_MAX: usize = 8;
const HELD_LABEL_PREFIX: &str = "Bill";
const HELD_ITEMS_SUFFIX: &str = "items";
const HELD_STRIP_LABEL: &str = "held";

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

#[derive(Clone, Copy, PartialEq)]
enum SaleView {
    New,
    Docs,
}

impl SaleView {
    const NEW_LABEL: &str = "New sale";
    const DOCS_LABEL: &str = "Documents";
}

const SCAN_PLACEHOLDER: &str = "scan or type barcode, Enter to add";
const UNKNOWN_BARCODE_PREFIX: &str = "unknown barcode: ";

#[derive(Clone, Copy, PartialEq)]
enum SettingsGroup {
    Payments,
    Team,
}

impl SettingsGroup {
    fn id(self) -> &'static str {
        match self {
            Self::Payments => "group-payments",
            Self::Team => "group-team",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Payments => "Payments",
            Self::Team => "Team",
        }
    }
    fn default_section(self) -> SettingsSection {
        match self {
            Self::Payments => SettingsSection::Receive,
            Self::Team => SettingsSection::Members,
        }
    }
    fn sections(self) -> &'static [SettingsSection] {
        match self {
            Self::Payments => &[
                SettingsSection::Receive,
                SettingsSection::AddAccount,
                SettingsSection::Tax,
            ],
            Self::Team => &[SettingsSection::Members, SettingsSection::Invite],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SettingsSection {
    Receive,
    AddAccount,
    Tax,
    Members,
    Invite,
}

impl SettingsSection {
    fn id(self) -> &'static str {
        match self {
            Self::Receive => "section-receive",
            Self::AddAccount => "section-add-account",
            Self::Tax => "section-tax",
            Self::Members => "section-members",
            Self::Invite => "section-invite",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Receive => "Receive accounts",
            Self::AddAccount => "Add account",
            Self::Tax => "Tax / VAT",
            Self::Members => "Members",
            Self::Invite => "Invite",
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

pub fn app_root() -> Element {
    let token = use_signal(|| Option::<String>::None);
    let base = use_signal(api_base);
    use_toast_provider();

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
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let busy = use_signal(|| false);
    let toast = use_toast();

    let do_auth = move |register: bool| {
        let client = FetchClient {
            base_url: base.clone(),
        };
        let mut busy = busy;
        let mut toast = toast.clone();
        let req = LoginRequest {
            email: email().clone(),
            password: password().clone(),
        };
        async move {
            busy.set(true);
            let res = if register {
                let reg = RegisterRequest {
                    email: req.email.clone(),
                    password: req.password.clone(),
                };
                match client.register(&reg).await {
                    Ok(_) => Ok(String::from("registered — now log in")),
                    Err(e) => Err(e),
                }
            } else {
                match client.login(&req).await {
                    Ok(r) => {
                        token.set(Some(r.token));
                        Ok(String::new())
                    }
                    Err(e) => Err(e),
                }
            };
            match res {
                Ok(msg) if msg.is_empty() => {}
                Ok(msg) => notify(&mut toast, &msg, false),
                Err(e) => notify(&mut toast, &e, true),
            }
            busy.set(false);
        }
    };

    let on_login = {
        let do_auth = do_auth.clone();
        move |_| {
            let fut = do_auth(false);
            spawn(async move {
                fut.await;
            });
        }
    };
    let on_register = move |_| {
        let fut = do_auth(true);
        spawn(async move {
            fut.await;
        });
    };

    rsx! {
        div { class: "auth",
            div { class: "brand",
                h1 { "solsulsol" }
                p { class: "dim", "tap goods · charge · get paid" }
            }
            div { class: "card form",
                input {
                    class: "field",
                    placeholder: "email",
                    value: "{email}",
                    oninput: move |e| email.set(e.value()),
                }
                input {
                    class: "field",
                    r#type: "password",
                    placeholder: "password",
                    value: "{password}",
                    oninput: move |e| password.set(e.value()),
                }
                button {
                    class: "btn primary full",
                    disabled: "{busy}",
                    onclick: on_login,
                    if busy() { "…" } else { "Login" }
                }
                button {
                    class: "btn ghost full",
                    disabled: "{busy}",
                    onclick: on_register,
                    if busy() { "…" } else { "Register" }
                }
            }
        }
    }
}

#[component]
fn Dashboard(token: Signal<Option<String>>, base: String) -> Element {
    use_modal_provider();
    let token_str = move || token().unwrap_or_default();
    let mut shop_id = use_signal(|| Option::<Uuid>::None);
    let shops = use_signal(Vec::<pos_api::schema::ShopResponse>::new);
    let mut new_shop = use_signal(String::new);
    let modal = use_modal();
    let toast = use_toast();

    let base_future = base.clone();
    use_future(move || {
        let client = FetchClient {
            base_url: base_future.clone(),
        };
        let mut shops = shops;
        let mut toast = toast.clone();
        let token = token;
        async move {
            match client.list_shops(&token().unwrap_or_default()).await {
                Ok(s) => shops.set(s),
                Err(e) => notify(&mut toast, &e, true),
            }
        }
    });

    let open_create = {
        let create_base = base.clone();
        move |_| {
            let mut modal = modal.clone();
            new_shop.set(String::new());
            modal.set_title("New shop");
            modal.set_content(rsx! {
                ShopForm {
                    token,
                    base: create_base.clone(),
                    shops,
                    name: new_shop,
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
                                onclick: move |_| shop_id.set(Some(s.shop_id)),
                                "{s.name}"
                            }
                        }
                    }
                    button {
                        class: "btn primary full",
                        "aria-label": "new-shop",
                        onclick: open_create,
                        "+ New shop"
                    }
                }
            }
            if let Some(sid) = shop_id() {
                ShopPanel { token: token_str(), base, shop_id: sid, token_signal: token }
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
) -> Element {
    let modal = use_modal();
    let toast = use_toast();
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
                onclick: move |_| {
                    let client = FetchClient { base_url: base.clone() };
                    let token = token;
                    let mut shops = shops;
                    let mut toast = toast.clone();
                    let mut modal = modal.clone();
                    let shop_name = name.read().clone();
                    spawn(async move {
                        match client.create_shop(&token().unwrap_or_default(), &CreateShopRequest { name: shop_name }).await {
                            Ok(_) => {
                                if let Ok(s) = client.list_shops(&token().unwrap_or_default()).await { shops.set(s); }
                                modal.close();
                            }
                            Err(e) => notify(&mut toast, &e, true),
                        }
                    });
                },
                "Create shop"
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
    let mut tab = use_signal(|| Tab::Sell);
    rsx! {
        div { class: "panel",
            Header { token_signal }
            main { class: "content",
                match tab() {
                    Tab::Sell => rsx! { SellTab { token, base, shop_id } },
                    Tab::Goods => rsx! { GoodsTab { token, base, shop_id } },
                    Tab::Reports => rsx! { ReportsTab { token, base, shop_id } },
                    Tab::Settings => rsx! { SettingsTab { token, base, shop_id } },
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
fn Header(token_signal: Signal<Option<String>>) -> Element {
    rsx! {
        header { class: "topbar",
            div { class: "brand-mini", Icon { path: ICON_SELL, size: 20 } }
            button {
                class: "icon-btn",
                "aria-label": "logout",
                onclick: move |_| token_signal.set(None),
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
    invoice: pos_api::schema::InvoiceResponse,
    expires_at: i64,
    pdf_url: String,
}

#[component]
fn SellTab(token: String, base: String, shop_id: Uuid) -> Element {
    let mut goods = use_signal(Vec::<pos_api::schema::GoodsResponse>::new);
    let mut destinations = use_signal(Vec::<pos_api::schema::DestinationResponse>::new);
    let mut selected_goods = use_signal(Vec::<Uuid>::new);
    let charge = use_signal(|| Option::<Charge>::None);
    let modal = use_modal();
    let mut now = use_signal(now_ms);
    let mut view = use_signal(|| SaleView::New);
    let mut docs = use_signal(Vec::<pos_api::schema::InvoiceResponse>::new);
    let mut held = use_signal(Vec::<Vec<Uuid>>::new);
    let mut scan_input = use_signal(String::new);
    let mut shop = use_signal(|| Option::<pos_api::schema::ShopResponse>::None);
    let mut toast = use_toast();

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
        }
    });

    use_future(move || async move {
        loop {
            now.set(now_ms());
            TimeoutFuture::new(TICK_MS as u32).await;
        }
    });

    let picked: Vec<pos_api::schema::GoodsResponse> = goods()
        .iter()
        .filter(|g| selected_goods().contains(&g.goods_id))
        .cloned()
        .collect();
    let subtotal: rust_decimal::Decimal = picked.iter().map(|g| g.price).sum();
    let shop_cfg = shop();
    let (vat_mode, vat_rate) = shop_cfg
        .map(|s| (s.vat_mode, s.vat_rate))
        .unwrap_or((pos_api::schema::VatMode::None, rust_decimal::Decimal::ZERO));
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
    let active_charge = charge().filter(|c| now() < c.expires_at);
    let expired = charge().is_some() && active_charge.is_none();
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
    let destination_id = destinations().first().map(|d| d.destination_id);

    rsx! {
        div { class: "stack",
            if active_charge.is_some() || expired {
                QrModalContent { charge, now }
            } else {
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
                            spawn(async move {
                                if let Ok(d) = client.list_invoices(&token, shop_id).await {
                                    docs.set(d);
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
                            match goods().iter().find(|g| {
                                g.barcode.as_deref() == Some(code.as_str())
                            }) {
                                Some(g) => {
                                    let mut cur = selected_goods().clone();
                                    if !cur.contains(&g.goods_id) {
                                        cur.push(g.goods_id);
                                        selected_goods.set(cur);
                                    }
                                }
                                None => {
                                    notify(
                                        &mut toast,
                                        &format!("{UNKNOWN_BARCODE_PREFIX}{code}"),
                                        true,
                                    );
                                }
                            }
                        },
                    }
                    p { class: "dim small", "type or scan a barcode to add — or tap the tiles" }
                }

                div { class: "stack",
                        div { class: "goods-grid",
                            for g in goods().iter().cloned() {
                                button {
                                    class: if selected_goods().contains(&g.goods_id) { "goods-tile picked" } else { "goods-tile" },
                                    onclick: move |_| {
                                        let mut cur = selected_goods().clone();
                                        if let Some(pos) = cur.iter().position(|x| *x == g.goods_id) { cur.remove(pos); } else { cur.push(g.goods_id); }
                                        selected_goods.set(cur);
                                    },
                                    span { class: "goods-img", Icon { path: ICON_GOODS, size: 24 } }
                                    span { class: "goods-name", "{g.name}" }
                                    if let Some(b) = &g.barcode {
                                        span { class: "dim small", "{b}" }
                                    }
                                    span { class: "goods-meta",
                                        span { class: "goods-price", "◎ {g.price}" }
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
                if total > rust_decimal::Decimal::ZERO {
                    div { class: "total-bar",
                        if vat_mode != pos_api::schema::VatMode::None {
                            span { class: "dim small",
                                "Subtotal \u{25c8}{subtotal} \u{00b7} VAT {vat_rate.normalize()}% \u{25c8}{vat_amount}" }
                        }
                        span { "Total" }
                        span { class: "pay-total small-total", "\u{25c8} {total}" }
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
                button {
                    class: "fab",
                    "aria-label": "new-doc",
                    disabled: "{selected_goods().is_empty() || destination_id.is_none()}",
                    onclick: {
                        let modal = modal.clone();
                        let base = base.clone();
                        let token = token.clone();
                        move |_| {
                            let goods_ids = selected_goods().clone();
                            let Some(destination_id) = destination_id else { return; };
                            let mut modal = modal.clone();
                            modal.set_title(BILLING_TITLE);
                            modal.set_content(rsx! {
                                CheckoutContent {
                                    shop_id,
                                    base: base.clone(),
                                    token: token.clone(),
                                    goods_ids,
                                    destination_id,
                                    selected_goods,
                                    goods,
                                    charge,
                                    now,
                                }
                            });
                            modal.open();
                        }
                    },
                    Icon { path: ICON_ADD, size: 28 }
                }
                    },
                    SaleView::Docs => rsx! {
                        DocumentsList { docs, base: base.clone() }
                    },
                }
            }
        }
    }
}

#[component]
fn DocumentsList(docs: Signal<Vec<pos_api::schema::InvoiceResponse>>, base: String) -> Element {
    rsx! {
        div { class: "stack",
            h2 { class: "page-title", "{SaleView::DOCS_LABEL}" }
            div { class: "list",
                for inv in docs().iter().cloned() {
                    div { class: "list-row",
                        div { class: "goods-col",
                            span { class: "goods-name", {inv.lines.first().map(|l| l.name.clone()).unwrap_or_default()} }
                            span { class: "dim small", "{chain_label(inv.destination.chain)} · margin ◎{inv.total - inv.cost}" }
                        }
                        div { class: "goods-col right",
                            span { class: "goods-price", "◎ {inv.total}" }
                            a { class: "dim small pdf-link", href: "{base}{inv.pdf_url}", target: "_blank", "PDF" }
                        }
                    }
                }
                if docs().is_empty() {
                    p { class: "dim pad", "no documents yet" }
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
    goods_ids: Vec<Uuid>,
    destination_id: Uuid,
    selected_goods: Signal<Vec<Uuid>>,
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
        let goods_ids = goods_ids.clone();
        async move {
            let client = FetchClient {
                base_url: base.clone(),
            };
            match client
                .checkout(
                    &token,
                    &CheckoutRequest {
                        shop_id,
                        goods_ids,
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
                        invoice: inv,
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
    let inv = &charge.invoice;
    let left = charge.expires_at - now_ms();
    let svg = qr_svg(&inv.qr_payload);
    let qr_el = match svg {
        Some(svg) => rsx! { div { class: "qr", dangerous_inner_html: "{svg}" } },
        None => rsx! { p { class: "error", "QR render failed" } },
    };
    let pdf_url = charge.pdf_url.clone();
    rsx! {
        div { class: "card pay",
            p { class: "pay-total", "◎ {inv.total}" }
            if inv.vat_mode != pos_api::schema::VatMode::None {
                p { class: "dim small", "Subtotal ◎{inv.subtotal} · VAT {inv.vat_rate.normalize()}% ◎{inv.vat_amount}" }
            }
            {qr_el}
            p { class: "pay-status",
                span { class: "pay-dot" }
                "waiting for payment"
            }
            p { class: if left < URGENCY_MS { "countdown urgent" } else { "countdown" }, "{fmt_remaining(left)}" }
            p { class: "dim small", "scan to pay · {chain_label(inv.destination.chain)} {truncate(&inv.destination.address, 8)}" }
            a { class: "btn ghost pdf-link", href: "{pdf_url}", target: "_blank", "Download PDF" }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n * 2 {
        return String::from(s);
    }
    format!("{}\u{2026}{}", &s[..n], &s[s.len() - n..])
}

fn chain_label(chain: pos_api::schema::Chain) -> &'static str {
    match chain {
        pos_api::schema::Chain::Solana => "Solana",
        pos_api::schema::Chain::Eth => "Ethereum",
        pos_api::schema::Chain::Btc => "Bitcoin",
        pos_api::schema::Chain::Binance => "Binance Pay",
        pos_api::schema::Chain::Bank => "Bank",
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
fn GoodsTab(token: String, base: String, shop_id: Uuid) -> Element {
    let modal = use_modal();
    let version = use_signal(|| 0u32);
    let mut goods = use_signal(Vec::<pos_api::schema::GoodsResponse>::new);
    let mut new_goods_name = use_signal(String::new);
    let mut new_goods_price = use_signal(String::new);
    let mut new_goods_cost = use_signal(String::new);
    let mut new_goods_units = use_signal(String::new);
    let mut new_goods_barcode = use_signal(String::new);

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

    let open_create = move |_| {
        let mut modal = modal.clone();
        new_goods_name.set(String::new());
        new_goods_price.set(String::new());
        new_goods_cost.set(String::new());
        new_goods_units.set(String::new());
        new_goods_barcode.set(String::new());
        modal.set_title("Create goods");
        modal.set_content(rsx! {
            GoodsForm {
                token: token.clone(),
                base: base.clone(),
                shop_id,
                name: new_goods_name,
                price: new_goods_price,
                cost: new_goods_cost,
                units: new_goods_units,
                barcode: new_goods_barcode,
                on_done: version,
            }
        });
        modal.open();
    };

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
            GoodsTable { goods }
        }
    }
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
fn GoodsTable(goods: ReadSignal<Vec<pos_api::schema::GoodsResponse>>) -> Element {
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
    let table = use_table(
        fetch,
        cols,
        Some(SortState {
            column: Some(String::from("name")),
            descending: false,
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
                                    }
                                },
                                "{col.head}"
                                if table.get_sort_col() == col.index {
                                    span { class: "sort-arrow", if table.get_sort_state() { "\u{25bc}" } else { "\u{25b2}" } }
                                }
                            }
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
                        }
                    }
                    if table.get_rows().is_empty() {
                        tr { td { class: "empty", colspan: "{GOODS_COLS.len()}", "no goods yet" } }
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
        "price" => rsx! { span { class: "goods-price", "◎ {row.0.price}" } },
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
    name: Signal<String>,
    price: Signal<String>,
    cost: Signal<String>,
    units: Signal<String>,
    barcode: Signal<String>,
    on_done: Signal<u32>,
) -> Element {
    let modal = use_modal();
    let toast = use_toast();
    rsx! {
        div { class: "stack",
            input {
                class: "field",
                placeholder: "name",
                value: "{name}",
                oninput: move |e| name.set(e.value()),
            }
            div { class: "row",
                input {
                    class: "field grow",
                    placeholder: "price",
                    value: "{price}",
                    oninput: move |e| price.set(e.value()),
                }
                input {
                    class: "field grow",
                    placeholder: "cost",
                    value: "{cost}",
                    oninput: move |e| cost.set(e.value()),
                }
                input {
                    class: "field units",
                    placeholder: "units",
                    value: "{units}",
                    oninput: move |e| units.set(e.value()),
                }
            }
            p { class: "dim small", "units 0 = stock not tracked" }
            input {
                class: "field",
                placeholder: "barcode (optional)",
                value: "{barcode}",
                oninput: move |e| barcode.set(e.value()),
            }
            p { class: "dim small", "EAN-13 / EAN-8 / UPC-A / Code39" }
            button {
                class: "btn primary full",
                "aria-label": "add-goods",
                onclick: move |_| {
                    let client = FetchClient { base_url: base.clone() };
                    let req = AddGoodsRequest {
                        shop_id,
                        name: name().clone(),
                        price: price().parse().unwrap_or_default(),
                        cost: cost().parse().unwrap_or_default(),
                        units: units().parse().unwrap_or_default(),
                        barcode: {
                            let b = barcode().trim().to_string();
                            if b.is_empty() { None } else { Some(b) }
                        },
                    };
                    let token = token.clone();
                    let mut toast = toast.clone();
                    let mut modal = modal.clone();
                    spawn(async move {
                        match client.add_goods(&token, &req).await {
                            Ok(_) => {
                                toast.success("goods created");
                                on_done.with_mut(|v| *v = v.wrapping_add(1));
                                modal.close();
                            }
                            Err(e) => toast.error(&e),
                        }
                    });
                },
                "Add goods"
            }
        }
    }
}

#[component]
fn SettingsTab(token: String, base: String, shop_id: Uuid) -> Element {
    let mut destinations = use_signal(Vec::<pos_api::schema::DestinationResponse>::new);
    let mut dest_chain = use_signal(|| Chain::Solana);
    let dest_token = use_signal(SplToken::default);
    let mut dest_address = use_signal(String::new);
    let mut members = use_signal(Vec::<pos_api::schema::ShopMemberResponse>::new);
    let mut invite_email = use_signal(String::new);
    let mut invite_role = use_signal(|| Role::Employee);
    let wallet = use_wallet();
    let wallet_busy = wallet.busy.cloned();
    let toast = use_toast();
    let mut group = use_signal(|| SettingsGroup::Payments);
    let mut section = use_signal(|| SettingsSection::Receive);
    let mut vat_mode = use_signal(|| VatMode::None);
    let mut vat_rate = use_signal(String::new);

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
            if let Ok(m) = client.list_members(&token, shop_id).await {
                members.set(m);
            }
            if let Some(s) = client
                .list_shops(&token)
                .await
                .ok()
                .and_then(|shops| shops.into_iter().find(|s| s.shop_id == shop_id))
            {
                vat_mode.set(s.vat_mode);
                vat_rate.set(s.vat_rate.normalize().to_string());
            }
        }
    });

    rsx! {
        div { class: "stack",
            h2 { class: "page-title", "Settings" }

            div { class: "settings-tabs",
                for g in [SettingsGroup::Payments, SettingsGroup::Team] {
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
                        SettingsSection::Receive => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Receive accounts" }
                                div { class: "list",
                                    for d in destinations().iter().cloned() {
                                        div { class: "list-row",
                                            div { class: "goods-col",
                                                span { class: "chain", "{chain_label(d.chain)}" }
                                                span { class: "mono dim small", "{truncate(&d.address, 10)}" }
                                            }
                                            span { class: "chip", "receives here" }
                                        }
                                    }
                                    if destinations().is_empty() {
                                        p { class: "dim pad", "no receive accounts yet — add one under Add account" }
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
                                                            let req = AddDestinationRequest { shop_id, chain: Chain::Solana, address: acct.pubkey, token: SplToken::Sol, label: Some(kind.name().to_string()) };
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
                                    value: "{dest_chain:?}",
                                    onchange: move |e| {
                                        dest_chain.set(match e.value().as_str() {
                                            "Eth" => Chain::Eth,
                                            "Btc" => Chain::Btc,
                                            "Binance" => Chain::Binance,
                                            "Bank" => Chain::Bank,
                                            _ => Chain::Solana,
                                        });
                                    },
                                    option { value: "Solana", "Solana (SOL)" }
                                    option { value: "Eth", "Ethereum (ETH)" }
                                    option { value: "Btc", "Bitcoin (BTC)" }
                                    option { value: "Binance", "Binance Pay (BNB)" }
                                    option { value: "Bank", "Bank account" }
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
                                                    _ => SplToken::Sol,
                                                });
                                            },
                                            option { value: "Sol", "Pay in SOL" }
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
                                p { class: "dim small", "wallet address · IBAN for bank accounts — shown as QR on every sale" }
                            }
                        },
                        SettingsSection::Tax => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Tax / VAT" }
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
                                    option { value: "None", "No VAT" }
                                    option { value: "Exclusive", "Exclusive — added on top" }
                                    option { value: "Inclusive", "Inclusive — included in price" }
                                }
                                if vat_mode() != VatMode::None {
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
                                    onclick: {
                                        let base = base.clone();
                                        let token = token.clone();
                                        let toast = toast.clone();
                                        move |_| {
                                            let client = FetchClient { base_url: base.clone() };
                                            let token = token.clone();
                                            let mut toast = toast.clone();
                                            let mode = vat_mode();
                                            let rate = vat_rate().parse::<rust_decimal::Decimal>().unwrap_or(rust_decimal::Decimal::ZERO);
                                            spawn(async move {
                                                let req = UpdateShopSettingsRequest { vat_mode: mode, vat_rate: rate };
                                                match client.update_shop_settings(&token, shop_id, &req).await {
                                                    Ok(_) => notify(&mut toast, "VAT settings saved", false),
                                                    Err(e) => notify(&mut toast, &e, true),
                                                }
                                            });
                                        }
                                    },
                                    "Save"
                                }
                                p { class: "dim small", "exclusive adds VAT on top of prices · inclusive extracts it — shown on every sale" }
                            }
                        },
                        SettingsSection::Members => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Team" }
                                div { class: "list",
                                    for member in members().iter().cloned() {
                                        div { class: "list-row",
                                            span { "{member.email}" }
                                            span { class: "chip", "{member.role:?}" }
                                        }
                                    }
                                    if members().is_empty() {
                                        p { class: "dim pad", "solo — invite your team under Invite" }
                                    }
                                }
                            }
                        },
                        SettingsSection::Invite => rsx! {
                            div { class: "card form",
                                h3 { class: "doc-title", "Invite teammate" }
                                div { class: "row",
                                    input {
                                        class: "field grow",
                                        placeholder: "email",
                                        value: "{invite_email}",
                                        oninput: move |e| invite_email.set(e.value()),
                                    }
                                    select {
                                        class: "field",
                                        value: "{invite_role:?}",
                                        onchange: move |e| {
                                            invite_role.set(match e.value().as_str() {
                                                "owner" => Role::Owner,
                                                "admin" => Role::Admin,
                                                _ => Role::Employee,
                                            });
                                        },
                                        option { value: "employee", "Employee" }
                                        option { value: "admin", "Admin" }
                                        option { value: "owner", "Owner" }
                                    }
                                    button {
                                        class: "btn primary",
                                        "aria-label": "invite",
                                        onclick: {
                                            let base = base.clone();
                                            let token = token.clone();
                                            let mut members = members;
                                            let toast = toast.clone();
                                            let mut section = section;
                                            move |_| {
                                                let client = FetchClient { base_url: base.clone() };
                                                let token = token.clone();
                                                let mut toast = toast.clone();
                                                let email = invite_email().clone();
                                                let role = invite_role();
                                                spawn(async move {
                                                    match client.invite(&token, &InviteRequest { shop_id, email, role }).await {
                                                        Ok(_) => {
                                                            invite_email.set(String::new());
                                                            if let Ok(m) = client.list_members(&token, shop_id).await { members.set(m); }
                                                            section.set(SettingsSection::Members);
                                                        }
                                                        Err(e) => notify(&mut toast, &e, true),
                                                    }
                                                });
                                            }
                                        },
                                        "+"
                                    }
                                }
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
    let mut invoices = use_signal(Vec::<pos_api::schema::InvoiceResponse>::new);
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
            match client.list_invoices(&token, shop_id).await {
                Ok(i) => invoices.set(i),
                Err(e) => notify(&mut toast, &e, true),
            }
        }
    });

    let zero = rust_decimal::Decimal::ZERO;
    let revenue = invoices().iter().map(|i| i.total).fold(zero, |a, t| a + t);
    let cost = invoices().iter().map(|i| i.cost).fold(zero, |a, t| a + t);
    let profit = revenue - cost;
    let rows: Vec<(String, pos_api::schema::InvoiceResponse)> = invoices()
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
                    span { class: "stat-value", "◎ {revenue}" }
                    span { class: "dim small", "{invoices().len()} sales" }
                }
                div { class: "card stat",
                    span { class: "stat-label", "Profit" }
                    span { class: "stat-value good", "◎ {profit}" }
                    span { class: "dim small", "after ◎{cost} cost" }
                }
            }
            h3 { class: "section-title", "Sales documents" }
            div { class: "list",
                for (name, inv) in rows.iter() {
                    div { class: "list-row",
                        div { class: "goods-col",
                            span { class: "goods-name", "{name}" }
                            span { class: "dim small", "{chain_label(inv.destination.chain)} · margin ◎{inv.total - inv.cost}" }
                        }
                        div { class: "goods-col right",
                            span { class: "goods-price", "◎ {inv.total}" }
                            a { class: "dim small pdf-link", href: "{base}{inv.pdf_url}", target: "_blank", "PDF" }
                        }
                    }
                }
                if invoices().is_empty() {
                    p { class: "dim pad", "no sales yet — complete one in the Sell tab" }
                }
            }
        }
    }
}

const STYLE: &str = "* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { background: #0d0b08; color: #f5efe8; font-family: system-ui, -apple-system, sans-serif; }
.app { min-height: 100vh; }
.dim { color: #8a7f73; }
.small { font-size: 12px; }
.mono { font-family: ui-monospace, monospace; }
.pad { padding: 16px; }
.grow { flex: 1; }
.error { color: #ff4d4d; font-size: 13px; padding: 8px 16px; }
.success { color: #58c98b; font-size: 13px; padding: 8px 16px; }
.row { display: flex; gap: 8px; }
.stack { display: flex; flex-direction: column; gap: 16px; }

.auth { min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 24px; padding: 24px; }
.brand { text-align: center; }
.brand h1 { font-size: 42px; letter-spacing: 8px; color: #ff7a00; }
.brand-mark { width: 72px; height: 72px; margin: 0 auto 12px; border-radius: 22px; background: #ff7a00; color: #0d0b08; display: flex; align-items: center; justify-content: center; }
.form { display: flex; flex-direction: column; gap: 10px; width: 100%; max-width: 360px; }
.card { background: #1a1512; border: 1px solid #26201a; border-radius: 14px; padding: 16px; }

.field { background: #0d0b08; border: 1px solid #26201a; border-radius: 10px; color: #f5efe8; padding: 12px 14px; font-size: 15px; outline: none; width: 100%; }
.field:focus { border-color: #ff7a00; }
select.field { appearance: none; }

.btn { border: none; border-radius: 10px; padding: 12px 18px; font-size: 15px; font-weight: 700; cursor: pointer; color: #f5efe8; background: #26201a; }
button:focus { outline: none; }
button:focus-visible { outline: 2px solid #ff7a00; outline-offset: 1px; }
.btn:disabled { opacity: 0.4; cursor: default; }
.btn.primary { background: #ff7a00; color: #16100a; }
.btn.ghost { background: transparent; border: 1px solid #3d2410; color: #ff7a00; }
.btn.full { width: 100%; }
.icon-btn { background: transparent; border: none; color: #8a7f73; cursor: pointer; padding: 6px; display: flex; align-items: center; }
.icon-btn:hover { color: #ff7a00; }

.shell { max-width: 480px; margin: 0 auto; min-height: 100vh; display: flex; flex-direction: column; }
.panel { display: flex; flex-direction: column; min-height: 100vh; }
.topbar { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; }
.brand-mini { color: #ff7a00; display: flex; }
.content { flex: 1; padding: 8px 16px 96px; }

.tabbar { position: fixed; bottom: 18px; left: 50%; transform: translateX(-50%); display: flex; gap: 6px; background: #1a1512; border: 1px solid #26201a; border-radius: 999px; padding: 8px 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.tab { background: transparent; border: none; color: #8a7f73; cursor: pointer; padding: 10px 18px; border-radius: 999px; display: flex; align-items: center; }
.tab.active { background: #ff7a00; color: #16100a; }

.shop-list { display: flex; flex-wrap: wrap; gap: 8px; }
.shop-chip { background: #0d0b08; border: 1px solid #26201a; color: #f5efe8; border-radius: 999px; padding: 8px 14px; cursor: pointer; font-size: 14px; }
.shop-chip.active { background: #3d2410; border-color: #ff7a00; color: #ff7a00; }

.goods-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.goods-tile { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; background: #1a1512; border: 1px solid #26201a; border-radius: 14px; padding: 14px; cursor: pointer; color: #f5efe8; text-align: left; }
.goods-tile.picked { border-color: #ff7a00; background: #3d2410; }
.goods-name { font-size: 14px; font-weight: 600; }
.goods-price { color: #ff7a00; font-weight: 700; font-size: 15px; }
.goods-meta { display: flex; align-items: center; gap: 8px; width: 100%; justify-content: space-between; }
.unit-chip { background: #26201a; color: #8a7f73; border-radius: 999px; padding: 2px 8px; font-size: 11px; font-weight: 700; }
.goods-tile.picked .unit-chip { color: #ff7a00; }

.page-title { font-size: 22px; font-weight: 800; letter-spacing: 0.5px; }
.doc-title { font-size: 13px; text-transform: uppercase; letter-spacing: 2px; color: #8a7f73; }

.list-row.goods { align-items: center; }
.goods-col { display: flex; flex-direction: column; gap: 3px; }
.goods-col.right { align-items: flex-end; }
.field.units { max-width: 80px; }

.dest-strip { display: flex; flex-wrap: wrap; gap: 8px; }
.dest-chip { background: #1a1512; border: 1px solid #26201a; color: #8a7f73; border-radius: 999px; padding: 8px 12px; cursor: pointer; font-size: 12px; font-family: ui-monospace, monospace; }
.dest-chip.active { border-color: #ff7a00; color: #ff7a00; background: #3d2410; }

.goods-img { width: 44px; height: 44px; border-radius: 10px; background: #26201a; color: #ff7a00; display: flex; align-items: center; justify-content: center; }

.shop-select { max-width: 420px; margin: 16vh auto 0; }
.onboard { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; margin-bottom: 8px; }

.settings-tabs { display: flex; gap: 6px; border-bottom: 1px solid #26201a; padding-bottom: 10px; }
.settings-tab { background: transparent; border: none; color: #8a7f73; cursor: pointer; padding: 8px 16px; border-radius: 999px; font-size: 14px; font-weight: 700; }
.settings-tab.active { background: #3d2410; color: #ff7a00; }
.settings-split { display: grid; grid-template-columns: 150px 1fr; gap: 12px; align-items: start; }
.settings-nav { display: flex; flex-direction: column; gap: 6px; }
.settings-item { background: transparent; border: 1px solid transparent; color: #8a7f73; cursor: pointer; padding: 10px 12px; border-radius: 10px; text-align: left; font-size: 13px; }
.settings-item.active { background: #1a1512; border-color: #26201a; color: #f5efe8; }
.settings-body { min-width: 0; }
@media (max-width: 560px) { .settings-split { grid-template-columns: 1fr; } .settings-nav { flex-direction: row; flex-wrap: wrap; } }

.fab { position: fixed; bottom: 92px; right: 20px; width: 62px; height: 62px; border-radius: 50%; background: #ff7a00; color: #16100a; border: none; cursor: pointer; display: flex; align-items: center; justify-content: center; box-shadow: 0 8px 24px rgba(0,0,0,0.5); z-index: 40; }
.fab:disabled { opacity: 0.4; cursor: default; }
.fab:active { transform: scale(0.96); }
.fab.hold { bottom: 166px; background: #26201a; color: #ff7a00; border: 1px solid #3d2410; box-shadow: none; }
.dest-chip.held-chip { color: #ff7a00; border-color: #3d2410; }

.pay { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 24px; }
.pay-total { font-size: 28px; font-weight: 800; color: #ff7a00; }
.qr { background: #fff; padding: 12px; border-radius: 12px; }
.qr svg { display: block; width: 220px; height: 220px; }
.countdown { font-size: 20px; font-weight: 700; font-variant-numeric: tabular-nums; }
.countdown.urgent { color: #ff4d4d; }
.expired { text-align: center; color: #ff4d4d; display: flex; flex-direction: column; gap: 12px; align-items: center; }

.list { display: flex; flex-direction: column; gap: 8px; }
.list-row { display: flex; align-items: center; justify-content: space-between; background: #1a1512; border: 1px solid #26201a; border-radius: 12px; padding: 12px 14px; font-size: 14px; }
.chip { background: #3d2410; color: #ff7a00; border-radius: 999px; padding: 3px 10px; font-size: 11px; text-transform: uppercase; font-weight: 700; }
.chain { font-weight: 700; font-size: 12px; color: #ff7a00; }
.field.price { max-width: 90px; }

.onboard { display: flex; flex-direction: column; gap: 6px; margin-bottom: 12px; }
.section-title { font-size: 13px; text-transform: uppercase; letter-spacing: 2px; color: #8a7f73; margin-top: 8px; }
.stats { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.stat { display: flex; flex-direction: column; gap: 4px; }
.stat-label { font-size: 12px; text-transform: uppercase; letter-spacing: 2px; color: #8a7f73; }
.stat-value { font-size: 26px; font-weight: 800; color: #ff7a00; }
.stat-value.good { color: #58c98b; }
.unit-chip.warn { color: #ffb84d; }
.goods-tile.soldout { opacity: 0.45; cursor: default; }
.pdf-link { text-decoration: none; }
a.pdf-link:hover { color: #ff7a00; }

.total-bar { position: fixed; bottom: 92px; left: 20px; right: 96px; display: flex; justify-content: space-between; align-items: center; background: #1a1512; border: 1px solid #26201a; border-radius: 999px; padding: 12px 20px; font-weight: 700; box-shadow: 0 8px 24px rgba(0,0,0,0.5); z-index: 39; }
.row.split { justify-content: space-between; align-items: center; }

.rtable { width: 100%; border-collapse: collapse; font-size: 14px; }
.rtable th { text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 1.5px; color: #8a7f73; padding: 8px 10px; border-bottom: 1px solid #26201a; cursor: pointer; user-select: none; white-space: nowrap; }
.rtable th.sortable:hover { color: #ff7a00; }
.sort-arrow { margin-left: 4px; color: #ff7a00; }
.rtable td { padding: 10px; border-bottom: 1px solid #1f1913; }
.rtable tr:last-child td { border-bottom: none; }
.rtable td.num { text-align: right; font-family: ui-monospace, monospace; }
.rtable td.empty { text-align: center; color: #8a7f73; padding: 20px; }
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
  .qr svg { width: 260px; height: 260px; }
}

.pay-status { display: flex; align-items: center; justify-content: center; gap: 8px; font-size: 13px; color: #8a7f73; }
.pay-dot { width: 8px; height: 8px; border-radius: 50%; background: #ff7a00; animation: pay-pulse 1.6s ease-in-out infinite; }
@keyframes pay-pulse { 0%, 100% { opacity: 0.25; } 50% { opacity: 1; } }
@media (max-width: 767px) {
  .fab { right: max(20px, calc(50% - 240px + 20px)); }
}

.toast-host { position: fixed; top: 16px; right: 16px; display: flex; flex-direction: column; align-items: flex-end; gap: 8px; z-index: 30; width: min(calc(100vw - 32px), 420px); pointer-events: none; }
.toast { pointer-events: none; display: flex; align-items: center; justify-content: space-between; gap: 12px; background: #1a1512; border: 1px solid #26201a; border-left: 4px solid #8a7f73; border-radius: 10px; padding: 10px 14px; font-size: 14px; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.toast .toast-close { pointer-events: auto; }
.toast.success { border-left-color: #58c98b; }
.toast.error { border-left-color: #ff4d4d; }
.toast.warning { border-left-color: #ffb84d; }
.toast.info { border-left-color: #ff7a00; }
.toast-close { background: transparent; border: none; color: #8a7f73; cursor: pointer; padding: 2px 4px; font-size: 12px; }
.toast-close:hover { color: #f5efe8; }

.modal-overlay { position: fixed; inset: 0; background: rgba(6, 4, 2, 0.72); display: flex; align-items: center; justify-content: center; padding: 20px; z-index: 50; }
.modal { position: relative; background: #1a1512; border: 1px solid #26201a; border-radius: 16px; padding: 20px; width: 100%; max-width: 400px; display: flex; flex-direction: column; gap: 14px; box-shadow: 0 16px 48px rgba(0,0,0,0.6); }
.modal-close { position: absolute; top: 10px; right: 10px; font-size: 20px; }
.center { align-items: center; }

@media (max-width: 560px) {
    .settings-split { grid-template-columns: 1fr; }
    .settings-nav { flex-direction: row; flex-wrap: wrap; }
}
";
