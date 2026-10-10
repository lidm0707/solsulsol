//! Dioxus hook wrapping wallet connect/sign state.

use dioxus::prelude::*;

use crate::wallet::{Wallet, WalletError, WalletKind};

#[derive(Clone)]
pub struct UseWallet {
    pub account: Signal<Option<WalletAccount>>,
    pub busy: Signal<bool>,
    pub error: Signal<Option<String>>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WalletAccount {
    pub kind: WalletKind,
    pub pubkey: String,
}

impl UseWallet {
    pub async fn connect(&mut self, kind: WalletKind) {
        self.error.set(None);
        let Some(wallet) = kind.detect() else {
            self.error
                .set(Some(WalletError::NotInstalled(kind.name()).to_string()));
            return;
        };
        self.busy.set(true);
        let res = wallet.connect().await;
        self.busy.set(false);
        match res {
            Ok(pubkey) => self.account.set(Some(WalletAccount { kind, pubkey })),
            Err(e) => self.error.set(Some(e.to_string())),
        }
    }

    pub fn disconnect(&mut self) {
        self.account.take();
        self.error.set(None);
    }

    /// Sign an arbitrary message with the connected wallet; returns the base58 signature.
    pub async fn sign_message(&mut self, message: &str) -> Option<String> {
        let Some(account) = self.account.read().clone() else {
            self.error.set(Some(String::from("wallet not connected")));
            return None;
        };
        let Some(wallet) = account.kind.detect() else {
            self.error.set(Some(String::from("wallet unavailable")));
            return None;
        };
        self.busy.set(true);
        let res = wallet.sign_message(message).await;
        self.busy.set(false);
        match res {
            Ok(sig) => Some(sig),
            Err(e) => {
                self.error.set(Some(e.to_string()));
                None
            }
        }
    }

    /// Sign + broadcast a base64 tx with the connected wallet; returns the signature.
    pub async fn sign_and_send(&mut self, tx_base64: &str) -> Option<String> {
        let Some(account) = self.account.read().clone() else {
            self.error.set(Some(String::from("wallet not connected")));
            return None;
        };
        let Some(wallet) = account.kind.detect() else {
            self.error.set(Some(String::from("wallet unavailable")));
            return None;
        };
        self.busy.set(true);
        let res = wallet.sign_and_send(tx_base64).await;
        self.busy.set(false);
        match res {
            Ok(sig) => Some(sig),
            Err(e) => {
                self.error.set(Some(e.to_string()));
                None
            }
        }
    }
}

pub fn use_wallet() -> UseWallet {
    UseWallet {
        account: use_signal(|| None),
        busy: use_signal(|| false),
        error: use_signal(|| None),
    }
}

/// Convenience: which wallets are installed in this browser.
pub fn installed_wallets() -> Vec<Wallet> {
    WalletKind::ALL.iter().filter_map(|k| k.detect()).collect()
}
