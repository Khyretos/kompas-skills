-- How a PC step was allowed, for the Activity tab: "allowed once · 10 min · removed after",
-- "always allowed · 24 h" or "standing grant".
ALTER TABLE pc_actions ADD COLUMN grant_note TEXT;
