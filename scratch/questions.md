# LOO-1065 review choices

The Task brief reads “ui polish dumping ground.” I chose the Mac New Task sheet and followed File and Start through the unstarted and running Task workspace. The recent workflow-preview change makes the filing mismatch visible in normal use.

For review, a named workflow is a choice for the immediate start. A filed Task follows the current default until someone starts it. Keeping a choice from File for later would introduce a durable pre-start preference and require deciding how it behaves when a Task moves Wave or a definition disappears. The chosen design uses the existing Task and Graph lifecycle.

The sheet opens the destination backlog after File succeeds, including when **In** changed. On a refused creation, retry becomes available once the original Machine confirms it has no receipt. A lost reply stays with creation recovery.

File reveals a hidden destination backlog and leaves another pane's zoom so the selected brief can appear. The mounted brief retires the saved submission through the same presentation hook as the Task workspace.

The built review keeps the backlog as File's destination and uses the committed row for its brief during reconnect. It keeps the sheet through admission so a refused Start can be corrected in place. If the chosen Wave disappears, the draft waits for a destination choice.

If planning beats the File reply and the Task has moved, open its brief in the backlog that now holds it. If it has entered Working, leave the current destination in place and keep the saved submission available to recovery.

While a selected Wave is unavailable, the workflow menu stays editable. Only File and Start wait for a destination.

A filter can hide a row during a drag. I kept the existing drop behavior for that case. The backlog accepts its own row and leaves the ranks alone.

Pass 2 choice. **In** selects the existing Wave ID. A rename keeps that choice and removal requires a new choice, even if a replacement has the same name. Use an object in the existing `wave` field for the ID and keep the legacy string for CLI name provisioning. A server that only understands strings can refuse the new shape safely.

When recovery confirms a refusal, **Edit draft** brings back the saved request on demand. An automatic reopening could interrupt work in another window.
