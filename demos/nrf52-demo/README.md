# `nrf52-demo`

This is a demo application we're using when hacking on Rubble. It runs on any
chip in the nRF52 family.
It exposes a dummy read-only battery service attribute, as well as a read/write attribute that
toggles an on-board LED (pin 17 by default).

The demo allows establishing a connection and provides a GATT server. A *lot*
of things are logged over RTT, which helps debugging.
