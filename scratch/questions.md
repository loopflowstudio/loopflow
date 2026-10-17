# LOO-415 assumptions

October 17, 2026: use a fixed five-second acknowledgement deadline after connecting
to the local owner socket. This bounds the identified silent-owner wait without a
new configuration knob; it does not bound SSH startup or an established attachment.
Cancellation uses the existing CLI signal handler and closes only the viewer.
