-- Per-folder fit mode (spec: "Amended after review (2026-08-12): per-folder
-- fit mode"). fit_to_page = 1 (default) scales content to fill the printable
-- area, upscaling if necessary. fit_to_page = 0 prints at natural size,
-- never enlarging -- but oversized content is still shrunk to fit either
-- way. The column is added to both tables because the flag is snapshotted
-- onto the job at enqueue time, like printer_name and copies, so editing a
-- folder never changes jobs already queued.
ALTER TABLE watch_folder ADD COLUMN fit_to_page INTEGER NOT NULL DEFAULT 1;
ALTER TABLE print_job    ADD COLUMN fit_to_page INTEGER NOT NULL DEFAULT 1;
