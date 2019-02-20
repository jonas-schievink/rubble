# WIP Rust BLE stack

This will hopefully eventually become a Bluetooth® Low Energy compatible
protocol stack.

**NOTE: None of this has passed the Bluetooth Qualification Process, so it must
not be used in commercial products!**

## Development

My development setup consists of a Nucleo-64 acting as an ST-Link programmer,
hooked up to the target board, which is a WT51822-S4AT containing an nRF51822.
You can find the latter for about 2-3 € on eBay if you look carefully. You
should also be able to find cheap ST-Link clones, but I can highly recommend
just buying a Nucleo board, they're very good and don't cost too much either.

The connections were made as follows:

```
ST-Link (CN4) | WT51822 |  nRF51822
 VDD_TARGET 1 |    -    |     -
      SWCLK 2 |   P11   | SWDCLK (24)
        GND 3 |   P2    | VSS
      SWDIO 4 |   P10   | SWDIO (23)
       NRST 5 |    -    |     -
        SWO 6 |    -    |     -

          3V3 |   P1    | VDD
```

ST-Link (CN4) is the CN4/SWD connector on the Nucleo board. Since this connector
does not provide a supply voltage (pin 1 just senses the target's voltage), the
3.3V supply voltage is obtained from the Arduino-style connector.

The openocd commandline that works for this setup is (tested under Linux only):

    openocd -f interface/stlink-v2-1.cfg -f target/nrf51.cfg
