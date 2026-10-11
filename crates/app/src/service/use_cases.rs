//! Wiring: concrete service struct holding port objects.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::config::PaymentConfig;
use crate::port::database::{
    DestinationRepo, GoodsRepo, InviteRepo, BillingRepo, NonceRepo, PaymentOrderRepo,
    ShopMemberRepo, ShopRepo, StorageUsageRepo, SubscriptionRepo, TxManager, UserRepo,
};
use crate::port::encryption::TokenIssuer;
use crate::port::object_storage::ObjectStorage;
use crate::port::promptpay::PromptPayEncoder;
use crate::port::qr::QrRenderer;
use crate::port::solana::{MessageVerifier, PaymentVerifier};

#[derive(Clone)]
pub struct AppServices {
    pub users: Arc<dyn UserRepo>,
    pub shops: Arc<dyn ShopRepo>,
    pub members: Arc<dyn ShopMemberRepo>,
    pub goods: Arc<dyn GoodsRepo>,
    pub destinations: Arc<dyn DestinationRepo>,
    pub invites: Arc<dyn InviteRepo>,
    pub billings: Arc<dyn BillingRepo>,
    pub nonces: Arc<dyn NonceRepo>,
    pub subscriptions: Arc<dyn SubscriptionRepo>,
    pub orders: Arc<dyn PaymentOrderRepo>,
    pub storage: Arc<dyn StorageUsageRepo>,
    pub tokens: Arc<dyn TokenIssuer>,
    pub wallet_verifier: Arc<dyn MessageVerifier>,
    pub payment_verifier: Arc<dyn PaymentVerifier>,
    pub qr: Arc<dyn QrRenderer>,
    pub promptpay: Arc<dyn PromptPayEncoder>,
    pub tx: Option<Arc<dyn TxManager>>,
    /// Optional blob storage; absent when the deployment configures none.
    pub objects: Option<Arc<dyn ObjectStorage>>,
    pub payment_config: PaymentConfig,
}

impl AppServices {
    pub fn builder(payment_config: PaymentConfig) -> AppServicesBuilder {
        AppServicesBuilder {
            payment_config: Some(payment_config),
            ..AppServicesBuilder::default()
        }
    }

    pub async fn in_tx<T>(
        &self,
        f: impl FnOnce() -> Pin<Box<dyn Future<Output = Result<T, String>> + Send>>,
    ) -> Result<T, String> {
        match &self.tx {
            Some(txm) => {
                let tx = txm.begin().await?;
                let result = f().await;
                match result {
                    Ok(v) => {
                        tx.commit().await?;
                        tracing::info!("transaction committed");
                        Ok(v)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "transaction rolled back");
                        tx.rollback().await?;
                        Err(e)
                    }
                }
            }
            None => f().await,
        }
    }
}

#[derive(Default)]
pub struct AppServicesBuilder {
    users: Option<Arc<dyn UserRepo>>,
    shops: Option<Arc<dyn ShopRepo>>,
    members: Option<Arc<dyn ShopMemberRepo>>,
    goods: Option<Arc<dyn GoodsRepo>>,
    destinations: Option<Arc<dyn DestinationRepo>>,
    invites: Option<Arc<dyn InviteRepo>>,
    billings: Option<Arc<dyn BillingRepo>>,
    nonces: Option<Arc<dyn NonceRepo>>,
    subscriptions: Option<Arc<dyn SubscriptionRepo>>,
    orders: Option<Arc<dyn PaymentOrderRepo>>,
    storage: Option<Arc<dyn StorageUsageRepo>>,
    tokens: Option<Arc<dyn TokenIssuer>>,
    wallet_verifier: Option<Arc<dyn MessageVerifier>>,
    payment_verifier: Option<Arc<dyn PaymentVerifier>>,
    qr: Option<Arc<dyn QrRenderer>>,
    promptpay: Option<Arc<dyn PromptPayEncoder>>,
    tx: Option<Arc<dyn TxManager>>,
    objects: Option<Arc<dyn ObjectStorage>>,
    payment_config: Option<PaymentConfig>,
}

macro_rules! setter {
    ($fn_name:ident, $field:ident, $ty:ident) => {
        pub fn $fn_name(mut self, v: Arc<dyn $ty>) -> Self {
            self.$field = Some(v);
            self
        }
    };
}

impl AppServicesBuilder {
    setter!(users, users, UserRepo);
    setter!(shops, shops, ShopRepo);
    setter!(members, members, ShopMemberRepo);
    setter!(goods, goods, GoodsRepo);
    setter!(destinations, destinations, DestinationRepo);
    setter!(invites, invites, InviteRepo);
    setter!(billings, billings, BillingRepo);
    setter!(nonces, nonces, NonceRepo);
    setter!(subscriptions, subscriptions, SubscriptionRepo);
    setter!(orders, orders, PaymentOrderRepo);
    setter!(storage, storage, StorageUsageRepo);
    setter!(tokens, tokens, TokenIssuer);
    setter!(wallet_verifier, wallet_verifier, MessageVerifier);
    setter!(payment_verifier, payment_verifier, PaymentVerifier);
    setter!(qr, qr, QrRenderer);
    setter!(promptpay, promptpay, PromptPayEncoder);
    setter!(tx, tx, TxManager);
    setter!(objects, objects, ObjectStorage);

    pub fn build(self) -> Result<AppServices, String> {
        const REQUIRED: &str = "missing required service";
        Ok(AppServices {
            users: self.users.ok_or(REQUIRED)?,
            shops: self.shops.ok_or(REQUIRED)?,
            members: self.members.ok_or(REQUIRED)?,
            goods: self.goods.ok_or(REQUIRED)?,
            destinations: self.destinations.ok_or(REQUIRED)?,
            invites: self.invites.ok_or(REQUIRED)?,
            billings: self.billings.ok_or(REQUIRED)?,
            nonces: self.nonces.ok_or(REQUIRED)?,
            subscriptions: self.subscriptions.ok_or(REQUIRED)?,
            orders: self.orders.ok_or(REQUIRED)?,
            storage: self.storage.ok_or(REQUIRED)?,
            tokens: self.tokens.ok_or(REQUIRED)?,
            wallet_verifier: self.wallet_verifier.ok_or(REQUIRED)?,
            payment_verifier: self.payment_verifier.ok_or(REQUIRED)?,
            qr: self.qr.ok_or(REQUIRED)?,
            promptpay: self.promptpay.ok_or(REQUIRED)?,
            tx: self.tx,
            objects: self.objects,
            payment_config: self.payment_config.ok_or("missing payment config")?,
        })
    }
}
