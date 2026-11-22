# Open questions

- `website/internal_pages.py` serves `/design`, `/colors` and `/fonts`: an older design system (Cormorant, Lato, font comparisons) that nothing links to and that disagrees with `DESIGN.md`. Delete them, along with their routes in `main.py`, so `DESIGN.md` and `design/prototypes/` are the only design system? Left in place: removing routes changes the product, which QA doesn't decide.
