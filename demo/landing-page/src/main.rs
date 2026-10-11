use dioxus::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    dioxus_web::launch::launch(App, vec![], vec![]);
}

const APP_URL: &str = match option_env!("LANDING_APP_URL") {
    Some(url) => url,
    None => "http://localhost:8081",
};

const ROADMAP_TITLE: &str = "Milestone";
const COMING_SOON_TITLE: &str = "Coming Soon — Revenue Share";

enum MilestoneState {
    Done,
    Now,
    Next,
}

impl MilestoneState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Now => "now",
            Self::Next => "next",
        }
    }
}

struct Milestone {
    state: MilestoneState,
    title: &'static str,
    detail: &'static str,
}

const MILESTONES: [Milestone; 5] = [
    Milestone {
        state: MilestoneState::Done,
        title: "Solana wallet auth",
        detail: "sign in with Phantom · Solflare",
    },
    Milestone {
        state: MilestoneState::Done,
        title: "POS + PromptPay QR",
        detail: "tap goods · charge · get paid",
    },
    Milestone {
        state: MilestoneState::Done,
        title: "SPL token payments",
        detail: "pay in the token you like",
    },
    Milestone {
        state: MilestoneState::Now,
        title: "Cloudflare Worker",
        detail: "same POS on D1, global edge",
    },
    Milestone {
        state: MilestoneState::Next,
        title: "Revenue share",
        detail: "earn from your platform activity",
    },
];

struct Feature {
    icon: &'static str,
    title: &'static str,
    body: &'static str,
}

const FEATURES: [Feature; 6] = [
    Feature {
        icon: ">_",
        title: "Goods Management",
        body: "Goods with icons, colors and sorting — manage your items right in the terminal.",
    },
    Feature {
        icon: "▚▞",
        title: "Scan & Pay",
        body: "Camera barcode/QR scanning in the browser. No native app.",
    },
    Feature {
        icon: "⇄",
        title: "Wallet Connect",
        body: "Phantom and other Solana wallets sign right from the terminal.",
    },
    Feature {
        icon: "↗",
        title: "Revenue Analytics",
        body: "Analyze your revenue live — share data and earn back by platform activity.",
    },
    Feature {
        icon: "▤",
        title: "PDF Receipts",
        body: "Printable receipts with embedded QR for every sale.",
    },
    Feature {
        icon: "▦",
        title: "PromptPay QR",
        body: "Thai PromptPay QR for every charge — fiat and crypto in one terminal.",
    },
];

#[component]
fn App() -> Element {
    rsx! {
        main { class: "page",
            div { class: "logo", "solsul", span { "sol" } }
            h1 { "Point of Sale that settles on ", em { "Solana" } }
            p { class: "tagline",
                "Billing, wallet payments, PromptPay and PDF receipts — "
                "one Rust terminal in your browser."
            }
            a { class: "btn primary", href: APP_URL, "Open the app" }
            div { class: "grid",
                for feature in FEATURES {
                    div { class: "card",
                        div { class: "icon", "{feature.icon}" }
                        h3 { "{feature.title}" }
                        p { "{feature.body}" }
                    }
                }
            }
            RoadmapView {}
            footer { "Rust · Dioxus · Solana" }
        }
    }
}

#[component]
fn RoadmapView() -> Element {
    let data: String = MILESTONES
        .iter()
        .map(|m| format!("{}~{}~{}", m.title, m.detail, m.state.as_str()))
        .collect::<Vec<_>>()
        .join(";");
    rsx! {
        div { class: "roadmap",
            h2 { class: "roadmap-title", "{ROADMAP_TITLE}" }
            div { id: "city-map",
                div { class: "map-ui",
                    div { class: "row",
                        button { id: "zoom-in", "+" }
                        button { id: "zoom-out", "\u{2212}" }
                        button { id: "zoom-reset", "\u{27f2}" }
                    }
                }
            }
            div { id: "city-map-data", class: "hidden-data", "{data}" }
            div { class: "card coming",
                span { class: "coming-badge", "COMING SOON" }
                h3 { class: "coming-title", "{COMING_SOON_TITLE}" }
                p { class: "coming-body",
                    "Share your data and open the app to earn. Agents pay via pay.sh to "
                    "summarize shop data — revenue flows back to active users, allocated "
                    "by platform activity."
                }
            }
        }
    }
}
