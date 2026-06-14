-- Roll back the two root categories added by up.sql.
-- Note: `asset_taxonomy_assignments` rows referencing these
-- category_ids cascade-delete via FK ON DELETE CASCADE.

DELETE FROM taxonomy_categories
 WHERE taxonomy_id = 'instrument_type'
   AND id IN ('INSURANCE_PRODUCT', 'PROVIDENT_FUND');
