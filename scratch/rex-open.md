MTG-409 was already moving. The originating session reported “cannot run Rex: No such file or directory” for both the Task and its URL. The login shell finds rex inside ~/Library/Application Support/com.superlogical.rex/RexServer/Versions/Current/Rex Server.app/Contents/MacOS. Neither ~/.local/bin nor the system bin directories contained rex.

With PATH=/usr/bin:/bin, the installed lf reproduced the same error against an unused socket. This session had REX_BIN_DIR set to the directory containing rex. Rex documents that variable in https://www.superlogical.com/rex/docs/automate/cli#path. A temporary ~/.local/bin/rex link confirmed the lookup failure and was removed.

In an isolated real-server check Rex reported the requested /var/folders checkout, while the provider's pwd -P reported the equivalent /private/var/folders path.
