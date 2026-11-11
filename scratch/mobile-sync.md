# Mobile current handoff · November 10

LOO-718, feature/demo, PR https://github.com/loopflowstudio/loopflow/pull/1812. Checkout /Users/jack/src/loopflow.mobile. Goal active; no merge, Auto-Merge or Task-completion authority. Current source84bbc89cbda6bf7061b991772dc953421ceebbe6. Published head51bbe6e723 completed full CI42931151790 SUCCESS; newer scroll changes still need publication/full CI. Fresh lf sync --manual changed no source.

## Current repair and phone

Actual phone input/submission reached this Session (“scroll”). Jack Heart: “i typed that from the phone”; “i see my text on the phone as i tye on mobile”; “but scrolling didnt work there”. Native regressions reproduced missing Latest after a swipe, output pulling the reader to the tail, and a taller Desktop screen replacing lines90–123 with170–203 while its outer viewport stayed still. Latest and deliberate reading now use both views' actual position; replying returns to following. Final46 terminal tests8.505s and three native checks passed; prior full Session suite10passed175.814s. Signed device build/signature passed. Details and failed/passing screenshots: mobile-swipe.md, mobile-desktop-scrolling.md; private reviewroot/desktop-scroll-final.

Installed signed phone artifact /Users/jack/.lf-machine/demos/mobile-718-live-84bbc89cb/LoopflowiOS.app. CoreDevice installation succeeded17:20:57 local; normal launch succeeded17:21:19 local, without fixture arguments. Device mobile heart: A7DD4746-FBC3-5016-889F-3E33FF3E98DD; Xcode00008130-0008751E3CC1001C, team2V3M244HF2, bundlecom.loopflow.ios.dev. Never launch with fixture arguments on the real phone.

Mac remains signed mobile-enabled311941bfe, installed normally over fixed main source. Protocol5, endpoint https://jacks-macbook-pro.tail0eda02.ts.net:53747, socket/private/tmp/lf-501-b5ed432959c9dad4/machine.sock. No Mac rebuild required for phone-only changes. Physical address admission, live Task/Session display, forwarded editing, passive background return, Back list continuity and portrait Find were performed. Actual phone submission was subsequently reported. Details: mobile-physical-qa.md, mobile-fixed-base.md.

## Next work

Publish current scroll repairs without auto-merge; follow exact-head CI. Verify physical scrolling, output during finger/deceleration, and return continuity. Back/reopen currently keeps Session choice and remote draft but recreates the renderer; reading-position restoration remains to be built. Compact-header/bottom-keyboard design is proposed in mobile-ux-study.html, not implemented. Research covers Rex, T3, Paseo and confirmed Paradev; source/docs/screenshots, no hands-on competitor use. Browser preview failed because no usable paired browser was available; HTML rendering is unverified.

Camera QR, cellular transitions, server replacement with an attached phone, microphone/CJK, VoiceOver and energy remain unchecked. Contextual approvals/Stop and automatic multi-Machine routing are absent. No iOS CLI, embedded Tailscale, relay, hub or new retention authority. Distribution/TestFlight belongs to LOO-721. Real Codex/Claude Simulator recovery/contextual reply checks passed; unresolved earlier TLS, UI automation initialization and missed Find dismissal failures are documented separately.

Reviewroot /Users/jack/.lf-machine/reviews/mobile-718-20261110. Latest logs and screenshot manifests: desktop-scroll-final/. Prior dated notes retain their original observations; historical handles and old installed-build claims are not current. Publication uses outside scratch archive, lf commit/lf scratch clear, prepared PR copy, lf pr publish, then restoration; never clear by hand.
