-- 015: rename invoices → billings (table, column, indexes) to match the
-- user-facing "billings" naming used by routes and UI.

ALTER TABLE invoices RENAME TO billings;
ALTER TABLE invoice_lines RENAME TO billing_lines;
ALTER INDEX idx_invoices_shop RENAME TO idx_billings_shop;
ALTER INDEX idx_lines_invoice RENAME TO idx_lines_billing;

ALTER TABLE billing_lines RENAME COLUMN invoice_id TO billing_id;
