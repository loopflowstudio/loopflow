# Assumptions — LOO-413

2026-10-07. Jack Heart authorized fixtures and isolated directories only. Provider
refresh-chain independence remains the first hand verification; implementation
cannot claim it from a second synthetic login. Real accounts remain untouched.

A provider is connected explicitly with `lf machine connect`; an account-selected
SSH launch invokes the same path when that account is absent. The target must
already be added. OpenCode and arbitrary Doppler environment forwarding are outside
the four-provider contract and are removed from SSH. Existing local API-key use
remains separate.

The previous broad design, rename evidence and unrelated later-slice assumptions
remain at `8f270beaf3cf752756bb0aaf254cf9a37dbc368d:scratch/`.

Jack Heart's October 7 steer assigns shared SSH transport, remote-install offers
and failure hints to LOO-411. This branch will integrate them, not implement them.

Jack Heart's comment `e332b3bb-603b-4dc4-bee4-1e5b704d5721` replaces `lf ssh`
with the global `lf --machine <label>` selector, without an alias. LOO-411 owns
that cutover. Its local and remote-tracking refs still point to `32607f1d24ad`
at this compress pass's first read; integrate the parent before publication.
