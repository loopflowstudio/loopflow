# LOO-803 choices

2026-11-08: Use exactly 30 seconds for “about 30 s”. Health starts at a
successful part reading, survives silence, and stops on an unavailable reading
or ended observation. Checkpoints alone do not establish part health. No new
timer is needed: assess the elapsed interval when choosing the next retry.
