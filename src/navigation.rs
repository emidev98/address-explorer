//! Screen navigation manifest: route, label, and icon in one place.
use crate::app::Route;
use crate::components::IconName;
/// One navigable destination of the application shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Addresses,
    FundsFlow,
    Exchanges,
    HistoricalPrices,
    SettingsGeneral,
    SettingsOnChainLinking,
    SettingsWalletTypes,
    SettingsExchangeTypes,
}
impl Destination {
    pub const PRIMARY: [Self; 4] = [
        Self::Addresses,
        Self::FundsFlow,
        Self::Exchanges,
        Self::HistoricalPrices,
    ];
    pub const SETTINGS: [Self; 4] = [
        Self::SettingsGeneral,
        Self::SettingsOnChainLinking,
        Self::SettingsWalletTypes,
        Self::SettingsExchangeTypes,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Addresses => "Addresses",
            Self::FundsFlow => "Funds Flow",
            Self::Exchanges => "Exchanges",
            Self::HistoricalPrices => "Historical Prices",
            Self::SettingsGeneral => "General",
            Self::SettingsOnChainLinking => "On-chain Linking",
            Self::SettingsWalletTypes => "Wallet Types",
            Self::SettingsExchangeTypes => "Exchange Types",
        }
    }
    pub const fn icon(self) -> IconName {
        match self {
            Self::Addresses => IconName::Addresses,
            Self::FundsFlow => IconName::Flow,
            Self::Exchanges => IconName::Exchanges,
            Self::HistoricalPrices => IconName::Prices,
            Self::SettingsGeneral
            | Self::SettingsOnChainLinking
            | Self::SettingsWalletTypes
            | Self::SettingsExchangeTypes => IconName::Settings,
        }
    }
    pub const fn route(self) -> Route {
        match self {
            Self::Addresses => Route::Addresses {},
            Self::FundsFlow => Route::FundsFlow {},
            Self::Exchanges => Route::Exchanges {},
            Self::HistoricalPrices => Route::HistoricalPrices {},
            Self::SettingsGeneral => Route::SettingsGeneral {},
            Self::SettingsOnChainLinking => Route::SettingsOnChainLinking {},
            Self::SettingsWalletTypes => Route::SettingsWalletTypes {},
            Self::SettingsExchangeTypes => Route::SettingsExchangeTypes {},
        }
    }
    pub const fn is_settings(self) -> bool {
        matches!(
            self,
            Self::SettingsGeneral
                | Self::SettingsOnChainLinking
                | Self::SettingsWalletTypes
                | Self::SettingsExchangeTypes
        )
    }
    /// Destination whose navigation entry is active for `route`.
    pub fn from_route(route: &Route) -> Option<Self> {
        match route {
            Route::Addresses {} => Some(Self::Addresses),
            Route::FundsFlow {} => Some(Self::FundsFlow),
            Route::Exchanges {} | Route::ExchangeAccountDetail { .. } => Some(Self::Exchanges),
            Route::HistoricalPrices {} => Some(Self::HistoricalPrices),
            Route::SettingsGeneral {} => Some(Self::SettingsGeneral),
            Route::SettingsOnChainLinking {} => Some(Self::SettingsOnChainLinking),
            Route::SettingsWalletTypes {} => Some(Self::SettingsWalletTypes),
            Route::SettingsExchangeTypes {} => Some(Self::SettingsExchangeTypes),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::Destination;
    use std::collections::HashSet;
    #[test]
    fn every_destination_has_a_unique_label() {
        let labels = Destination::PRIMARY
            .into_iter()
            .chain(Destination::SETTINGS)
            .map(Destination::label)
            .collect::<HashSet<_>>();
        assert_eq!(labels.len(), 8);
    }
}
