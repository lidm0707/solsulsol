//! Wiring: concrete service struct holding port objects.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::port::{
    DestinationRepo, InviteRepo, InvoiceRepo, PasswordHasher, ProductRepo, PromptPayEncoder,
    QrRenderer, ShopMemberRepo, ShopRepo, TokenIssuer, TxManager, UserRepo,
};

#[derive(Clone)]
pub struct AppServices {
    pub users: Arc<dyn UserRepo>,
    pub shops: Arc<dyn ShopRepo>,
    pub members: Arc<dyn ShopMemberRepo>,
    pub products: Arc<dyn ProductRepo>,
    pub destinations: Arc<dyn DestinationRepo>,
    pub invites: Arc<dyn InviteRepo>,
    pub invoices: Arc<dyn InvoiceRepo>,
    pub hasher: Arc<dyn PasswordHasher>,
    pub tokens: Arc<dyn TokenIssuer>,
    pub qr: Arc<dyn QrRenderer>,
    pub promptpay: Arc<dyn PromptPayEncoder>,
    pub tx: Option<Arc<dyn TxManager>>,
}

impl AppServices {
    pub fn builder() -> AppServicesBuilder {
        AppServicesBuilder::default()
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
    products: Option<Arc<dyn ProductRepo>>,
    destinations: Option<Arc<dyn DestinationRepo>>,
    invites: Option<Arc<dyn InviteRepo>>,
    invoices: Option<Arc<dyn InvoiceRepo>>,
    hasher: Option<Arc<dyn PasswordHasher>>,
    tokens: Option<Arc<dyn TokenIssuer>>,
    qr: Option<Arc<dyn QrRenderer>>,
    promptpay: Option<Arc<dyn PromptPayEncoder>>,
    tx: Option<Arc<dyn TxManager>>,
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
    setter!(products, products, ProductRepo);
    setter!(destinations, destinations, DestinationRepo);
    setter!(invites, invites, InviteRepo);
    setter!(invoices, invoices, InvoiceRepo);
    setter!(hasher, hasher, PasswordHasher);
    setter!(tokens, tokens, TokenIssuer);
    setter!(qr, qr, QrRenderer);
    setter!(promptpay, promptpay, PromptPayEncoder);
    setter!(tx, tx, TxManager);

    pub fn build(self) -> Result<AppServices, String> {
        const REQUIRED: &str = "missing required service";
        Ok(AppServices {
            users: self.users.ok_or(REQUIRED)?,
            shops: self.shops.ok_or(REQUIRED)?,
            members: self.members.ok_or(REQUIRED)?,
            products: self.products.ok_or(REQUIRED)?,
            destinations: self.destinations.ok_or(REQUIRED)?,
            invites: self.invites.ok_or(REQUIRED)?,
            invoices: self.invoices.ok_or(REQUIRED)?,
            hasher: self.hasher.ok_or(REQUIRED)?,
            tokens: self.tokens.ok_or(REQUIRED)?,
            qr: self.qr.ok_or(REQUIRED)?,
            promptpay: self.promptpay.ok_or(REQUIRED)?,
            tx: self.tx,
        })
    }
}
