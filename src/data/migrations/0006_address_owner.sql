-- Extend addresses with a free-form description and an optional owner
-- classification referencing the lookup tables created by migration 0005.
--
-- A stored address belongs to either an exchange or a wallet, never both, and
-- may also be unclassified. ALTER TABLE ADD COLUMN cannot add a table-level
-- CHECK constraint across columns, so the either-or invariant is enforced with
-- triggers and repeated by the address repository before writing.

ALTER TABLE addresses ADD COLUMN description TEXT;
ALTER TABLE addresses ADD COLUMN exchange_id INTEGER REFERENCES exchanges_types(id);
ALTER TABLE addresses ADD COLUMN wallet_id INTEGER REFERENCES wallets_types(id);

-- Reverse transaction lookups scan input prevouts and outputs of the
-- transactions stored for known addresses; both address columns need indexes
-- so the search stays fast as the transaction store grows.
CREATE INDEX IF NOT EXISTS idx_transaction_outputs_scriptpubkey_address
    ON transaction_outputs(scriptpubkey_address);

CREATE INDEX IF NOT EXISTS idx_transaction_inputs_prevout_scriptpubkey_address
    ON transaction_inputs(prevout_scriptpubkey_address);

CREATE TRIGGER IF NOT EXISTS addresses_owner_either_insert
BEFORE INSERT ON addresses
WHEN NEW.exchange_id IS NOT NULL AND NEW.wallet_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'addresses.exchange_id and addresses.wallet_id cannot both be set');
END;

CREATE TRIGGER IF NOT EXISTS addresses_owner_either_update
BEFORE UPDATE OF exchange_id, wallet_id ON addresses
WHEN NEW.exchange_id IS NOT NULL AND NEW.wallet_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'addresses.exchange_id and addresses.wallet_id cannot both be set');
END;
