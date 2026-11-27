-- Notes are limited to 500 characters; longer ones are cut so every stored note still reads back.
ALTER TABLE contacts DROP CONSTRAINT contacts_notes_length;
UPDATE contacts SET notes = left(notes, 500) WHERE char_length(notes) > 500;
ALTER TABLE contacts ADD CONSTRAINT contacts_notes_length CHECK (notes IS NULL OR char_length(notes) <= 500);
