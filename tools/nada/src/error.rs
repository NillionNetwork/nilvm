//! Error utilities
use eyre::eyre;
use nillion_client::operation::{InitialStateInvokeError, InvokeError};

pub(crate) trait IntoEyre<T> {
    fn into_eyre(self) -> eyre::Result<T>;
}

impl<T> IntoEyre<T> for Result<T, anyhow::Error> {
    fn into_eyre(self) -> eyre::Result<T> {
        self.map_err(|e| eyre!("{e:?}"))
    }
}

impl<T> IntoEyre<T> for Result<T, InitialStateInvokeError> {
    fn into_eyre(self) -> eyre::Result<T> {
        self.map_err(|e| eyre!("{e:?}"))
    }
}

impl<T> IntoEyre<T> for Result<T, InvokeError> {
    fn into_eyre(self) -> eyre::Result<T> {
        self.map_err(|e| eyre!("{e:?}"))
    }
}
