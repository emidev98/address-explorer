-- Normalize legacy BTC/XBT fees imported in whole-BTC units to satoshis.
--
-- Several export parsers stored the source `fee` column verbatim even when it
-- was denominated in BTC/XBT, while the rest of the application treats the
-- `fee` column as satoshis for those currencies. A fractional BTC/XBT fee
-- cannot be satoshis, so every non-integer value is a legacy source-unit value.
UPDATE exchange_transactions
SET fee = ROUND(fee * 100000000.0)
WHERE UPPER(TRIM(COALESCE(fee_currency, ''))) IN ('BTC', 'XBT')
  AND fee IS NOT NULL
  AND fee <> ROUND(fee);

-- `exchange_balance_sats` subtracts BTC/XBT fees from the ledger movement, so
-- the repaired fees change the derived provider balance. Rebuild it with the
-- same formula as `refresh_provider_balance`, assigning the total to each
-- provider's alphabetically first address.
UPDATE addresses
SET exchange_balance_sats = 0
WHERE exchange_id IS NOT NULL;

UPDATE addresses
SET exchange_balance_sats = (
    SELECT CAST(ROUND(COALESCE(SUM(
        CASE t.tx_type
            WHEN 'buy' THEN ABS(t.amount_sats)
            WHEN 'deposit' THEN ABS(t.amount_sats)
            WHEN 'reward' THEN ABS(t.amount_sats)
            WHEN 'sell' THEN -ABS(t.amount_sats)
            WHEN 'withdrawal' THEN -ABS(t.amount_sats)
            ELSE t.amount_sats
        END
        - CASE WHEN UPPER(TRIM(COALESCE(t.fee_currency, ''))) IN ('BTC', 'XBT')
               THEN ABS(COALESCE(t.fee, 0)) ELSE 0 END
    ), 0)) AS INTEGER)
    FROM exchange_transactions t
    JOIN exchange_accounts a ON a.id = t.exchange_id
    WHERE a.provider_id = addresses.exchange_id
)
WHERE exchange_id IS NOT NULL
  AND address = (
      SELECT MIN(a2.address)
      FROM addresses a2
      WHERE a2.exchange_id = addresses.exchange_id
  );

-- Invariant from here on: a BTC/XBT fee is stored as integer satoshis.
CREATE TRIGGER IF NOT EXISTS exchange_transactions_btc_fee_sats_insert
BEFORE INSERT ON exchange_transactions
WHEN UPPER(TRIM(COALESCE(NEW.fee_currency, ''))) IN ('BTC', 'XBT')
  AND NEW.fee IS NOT NULL
  AND NEW.fee <> ROUND(NEW.fee)
BEGIN
    SELECT RAISE(ABORT, 'exchange_transactions.fee must be satoshis for BTC/XBT');
END;

CREATE TRIGGER IF NOT EXISTS exchange_transactions_btc_fee_sats_update
BEFORE UPDATE OF fee, fee_currency ON exchange_transactions
WHEN UPPER(TRIM(COALESCE(NEW.fee_currency, ''))) IN ('BTC', 'XBT')
  AND NEW.fee IS NOT NULL
  AND NEW.fee <> ROUND(NEW.fee)
BEGIN
    SELECT RAISE(ABORT, 'exchange_transactions.fee must be satoshis for BTC/XBT');
END;
