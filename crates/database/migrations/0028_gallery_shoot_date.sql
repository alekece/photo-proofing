-- The day of the shoot, as the photographer enters it; NULL when not given. A calendar day with
-- no time or zone, shown only to the photographer.
ALTER TABLE galleries ADD COLUMN shoot_date date;
