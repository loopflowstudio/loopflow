# Remaining verification

Jack Heart: “Lets get this done?”

Keep both pieces in #1950. Mac screen acquisition is a visual-observation upper
bound; phone consumes the next ReplayKit screen frame during an explicit session.
Neither claims the compositor's first-present time. No bitmaps are written to disk.
Missing grants, disabled sessions or invalid clocks remain unmeasured.

Verify final builds, real native capture and app-journal trials. Code-level checks
cover UIKit event dispatch, visual ordering/supersession and request-isolated SQL.
