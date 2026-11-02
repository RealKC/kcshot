CREATE VIEW IF NOT EXISTS screenshots_ordered AS
    SELECT row_number() OVER (ORDER BY id DESC) - 1 AS idx, *
    FROM screenshots;
