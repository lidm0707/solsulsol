-- Goods barcode: optional scanner symbology (EAN-13/8, UPC-A, Code39).

ALTER TABLE products
    ADD COLUMN IF NOT EXISTS barcode TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS products_shop_barcode_idx
    ON products (shop_id, barcode)
    WHERE barcode IS NOT NULL;
