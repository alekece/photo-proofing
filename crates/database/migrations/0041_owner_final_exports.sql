-- Owner final snapshots are independent of both favorites and client delivery.
ALTER TABLE exports
    DROP CONSTRAINT exports_kind_known,
    ADD CONSTRAINT exports_kind_known CHECK (kind IN ('selection', 'final_selection', 'delivery'));
