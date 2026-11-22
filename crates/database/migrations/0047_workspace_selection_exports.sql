-- Older single-gallery snapshots already identify the source through their export.
ALTER TABLE export_photos ADD COLUMN source_gallery_id uuid;

ALTER TABLE exports
    DROP CONSTRAINT exports_kind_known,
    ADD CONSTRAINT exports_kind_known CHECK (kind IN ('selection', 'final_selection', 'delivery', 'workspace_selection'));

CREATE UNIQUE INDEX exports_one_workspace_selection_preparing ON exports(workspace_id)
    WHERE kind = 'workspace_selection' AND status ->> 'kind' = 'preparing';
