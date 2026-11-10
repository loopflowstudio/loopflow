# Creation presentation

Opening a Task currently removes its saved submission before SwiftUI presents the destination. A dismissed or replaced sheet can therefore lose recovery without showing the Task. Keep submission/outcome until the destination appears; acknowledge the exact original submission against the latest local draft so a newer draft survives. Each sheet owns its model identity; only the still-presented sheet may navigate or dismiss. Cancellation is immediate on Close and also applies on disappearance. Read/recovery never repeats creation.

Verification: focused iOS creation tests, including read without presentation across reload, late presentation with a newer draft, repeat acknowledgment, and canceled reads. Real-phone setup remains ready on the Mac and awaiting phone unlock.

November10 result: focused native iOS creation suite18passed in8.69s; build succeeded, disposable Simulator cleaned. Committed locally b9bb6791d83bcb7cf19aa3a39c321627a7f26d4e, not pushed. Signed device build succeeded; signature verified. New phone artifact retained at `/Users/jack/.lf-machine/demos/mobile-718-live-b9bb6791d/LoopflowiOS.app`, not installed/launched. /tmp/mobile-creation-phone-build.log. Mac server/listener unchanged and ready; phone unlock still pending. Model recovery/cancellation and leaf views tested; physical-phone presentation and terminal input remain unverified.
