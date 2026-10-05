-- Item 7: each user's card colours and labels per action type (JSON: {"read": {"label": "Read", "color": "#5c398e"}, ...}); {} = the defaults.
ALTER TABLE users ADD COLUMN card_style TEXT NOT NULL DEFAULT '{}';
