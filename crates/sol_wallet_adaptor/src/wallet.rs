//! Browser wallet detection, connect and transaction signing (Phantom, Solflare).

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const PROVIDER_PHANTOM: &str = "solana";
const PROVIDER_SOLFLARE: &str = "solflare";
const METHOD_CONNECT: &str = "connect";
const METHOD_SIGN_SEND: &str = "signAndSendTransaction";
const FIELD_ADDRESS: &str = "toString";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletKind {
    Phantom,
    Solflare,
}

impl WalletKind {
    pub const ALL: [Self; 2] = [Self::Phantom, Self::Solflare];

    pub fn name(self) -> &'static str {
        match self {
            Self::Phantom => "Phantom",
            Self::Solflare => "Solflare",
        }
    }

    fn window_key(self) -> &'static str {
        match self {
            Self::Phantom => PROVIDER_PHANTOM,
            Self::Solflare => PROVIDER_SOLFLARE,
        }
    }

    pub fn detect(self) -> Option<Wallet> {
        let provider = provider(self)?;
        if !provider.is_phantom() && self == Self::Phantom {
            return None;
        }
        Some(Wallet {
            kind: self,
            provider,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("wallet {0} not installed")]
    NotInstalled(&'static str),
    #[error("wallet request rejected")]
    Rejected,
    #[error("wallet error: {0}")]
    Js(String),
}

impl WalletError {
    fn from_js(err: JsValue) -> Self {
        match err.as_string() {
            Some(s) if s.contains("reject") || s.contains("denied") => Self::Rejected,
            Some(s) => Self::Js(s),
            None => Self::Js(format!("{err:?}")),
        }
    }
}

#[derive(Clone)]
pub struct Wallet {
    kind: WalletKind,
    provider: Provider,
}

impl Wallet {
    pub fn kind(&self) -> WalletKind {
        self.kind
    }

    pub async fn connect(&self) -> Result<String, WalletError> {
        call_async(&self.provider, METHOD_CONNECT, &[]).await?;
        let account = account_of(&self.provider);
        account.ok_or(WalletError::Rejected)
    }

    pub async fn disconnect(&self) -> Result<(), WalletError> {
        call_async(&self.provider, "disconnect", &[]).await?;
        Ok(())
    }

    /// Sign and broadcast a base64-encoded serialized `Transaction`/`VersionedTransaction`
    /// (built off-chain, e.g. by the backend) and return the signature.
    pub async fn sign_and_send(&self, tx_base64: &str) -> Result<String, WalletError> {
        let arg = js_sys::Uint8Array::from(crate::wallet::decode_base64(tx_base64).as_slice());
        let sig = call_async(&self.provider, METHOD_SIGN_SEND, &[arg.into()]).await?;
        signature_of(&sig).ok_or(WalletError::Rejected)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone)]
    pub type Provider;

    #[wasm_bindgen(method, getter, js_name = isPhantom)]
    fn is_phantom(this: &Provider) -> bool;

    #[wasm_bindgen(method, getter, js_name = publicKey)]
    fn public_key(this: &Provider) -> JsValue;
}

fn provider(kind: WalletKind) -> Option<Provider> {
    let window = web_sys::window()?;
    let any: &js_sys::Object = window.as_ref();
    js_sys::Reflect::get(any, &JsValue::from_str(kind.window_key()))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
        .map(Provider::from)
}

fn account_of(provider: &Provider) -> Option<String> {
    let pk = provider.public_key();
    if pk.is_undefined() || pk.is_null() {
        return None;
    }
    let obj = pk.dyn_into::<js_sys::Object>().ok()?;
    let to_string = js_sys::Reflect::get(&obj, &JsValue::from_str(FIELD_ADDRESS)).ok()?;
    let f = to_string.dyn_into::<js_sys::Function>().ok()?;
    f.call0(&obj).ok()?.as_string()
}

async fn call_async(
    this: &Provider,
    method: &str,
    args: &[JsValue],
) -> Result<JsValue, WalletError> {
    let f = js_sys::Reflect::get(this, &JsValue::from_str(method))
        .map_err(|_| WalletError::NotInstalled(""))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| WalletError::NotInstalled(""))?;
    let ret = f
        .apply(this, &js_sys::Array::from_iter(args.iter().cloned()))
        .map_err(WalletError::from_js)?;
    let promise = js_sys::Promise::from(ret);
    JsFuture::from(promise).await.map_err(WalletError::from_js)
}

fn signature_of(value: &JsValue) -> Option<String> {
    if let Some(s) = value.as_string() {
        return Some(s);
    }
    let obj = value.dyn_ref::<js_sys::Object>()?;
    let sig = js_sys::Reflect::get(obj, &JsValue::from_str("signature")).ok()?;
    sig.as_string()
}

/// Minimal standard base64 decode for wasm payloads (padding-tolerant).
fn decode_base64(input: &str) -> Vec<u8> {
    const INVALID: u8 = u8::MAX;
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [INVALID; 256];
    let mut i = 0;
    while i < 64 {
        table[ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    table[b'-' as usize] = 62;
    table[b'_' as usize] = 63;

    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in input.as_bytes() {
        let v = table[b as usize];
        if v == INVALID {
            continue;
        }
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}
